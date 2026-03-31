# Edge Cases — Comprehensive Catalog

Every edge case that can break Phantom, and the exact solution for each.

---

## Input Capture Edge Cases

### Key Repeat Events

**Problem:** Linux input subsystem generates repeat events (`value: 2`) when a key is held. Processing these causes phantom touches (pun intended).

**Solution:** In the evdev event parser, reject any `EV_KEY` event where `value == 2`. Only process `1` (press) and `0` (release). This check is in `input.rs`, first filter applied.

```
if event.type == EV_KEY && event.value == 2 {
    continue; // ignore repeat
}
```

### SYN_DROPPED Events

**Problem:** When the kernel's input buffer overflows (fast mouse, slow reader), it sends a `SYN_DROPPED` event to signal data loss. Continued reads return garbage until the next sync.

**Solution:** On `SYN_DROPPED`, stop processing events from that device fd. Call `ioctl(EVIOCGKEY, ...)` to re-read the current key state and resync the engine. Then resume normal event reading.

### Multiple Keyboards / Mice

**Problem:** User has multiple keyboards or mice plugged in. Each generates independent events.

**Solution:** Grab ALL matching devices. Events from any grabbed device feed into the same keymap engine. A keypress on keyboard A and keyboard B both map to the same key bindings. No distinction needed.

### Device Disappears Mid-Session

**Problem:** User unplugs USB keyboard while gaming. The fd becomes invalid.

**Solution:** epoll returns `EPOLLHUP` or reads return `ENODEV`. Remove the fd from the epoll set, log a warning. Continue running with remaining devices. If the mouse disappears, camera control stops but other nodes still work.

### Device Appears Mid-Session

**Problem:** User plugs in a new keyboard while gaming.

**Solution:** **Not handled.** Deliberate simplicity choice. Daemon only scans devices at startup. If user needs a new device, restart the daemon. Attempting hotplug adds significant complexity (udev monitor, race conditions, device identity tracking).

### EVIOCGRAB Fails

**Problem:** Another process already grabbed the device (e.g., another instance of Phantom, or a VM).

**Solution:** Log error with device path, skip that device, continue with remaining devices. If ALL devices fail to grab, exit with a clear error message listing the conflicting processes (parse `/proc/*/fd/` to find who holds the device).

---

## Mouse Edge Cases

### High-DPI Gaming Mice

**Problem:** A 16000 DPI mouse generates huge delta values per event. Combined with high polling rate (1000Hz), the camera moves too fast.

**Solution:** Sensitivity multiplier in the profile. The `sensitivity` field on `mouse_camera` nodes scales the delta. User tunes this per-game. Default is `1.0` which should be usable for 800 DPI mice. High-DPI users set it to 0.1–0.3.

### Mouse Axis Reversal

**Problem:** Some mice or configurations report inverted Y axis.

**Solution:** The `invert_y` field on `mouse_camera` nodes. When `true`, negate the Y delta before applying it. This is per-node, so different camera nodes can have different inversion settings.

### Mouse Events Without Movement

**Problem:** Mouse button events (left click, right click) arrive on the mouse device alongside movement events. These must not interfere with camera tracking.

**Solution:** Filter mouse device events: only process `EV_REL` events (`REL_X`, `REL_Y`). Button events (`EV_KEY` with `BTN_LEFT`, etc.) are processed by the keymap engine as key bindings, not as camera input. The same physical mouse generates both movement (for camera) and button presses (for hold_tap nodes) — this is correct.

### Mouse Wheel Events

**Problem:** Scroll wheel generates `EV_REL REL_WHEEL` events. These should be mappable as keys.

**Solution:** Treat `REL_WHEEL` as a special input source. Map `WheelUp` and `WheelDown` as key names in profiles. When `REL_WHEEL` value is positive, fire `WheelUp`; when negative, fire `WheelDown`. This allows scroll wheel to be bound to zoom, weapon switch, etc.

---

## Touch Injection Edge Cases

### Write to uinput Fails

**Problem:** The kernel's input event buffer is full (extremely rapid writes). `write()` returns `EAGAIN`.

**Solution:** Retry with a 1ms sleep, up to 3 times. If still failing, drop the event and log a warning. This should never happen at realistic event rates (even 1000Hz mouse = 1000 events/sec is well within kernel capacity).

### uinput Device Already Exists

**Problem:** Previous daemon instance was SIGKILL'd, leaving an orphaned uinput device.

**Solution:** On startup, check `/proc/bus/input/devices` for "Phantom Virtual Touch". If found, attempt to open and destroy it via `UI_DEV_DESTROY`. Then create a fresh device. If destroy fails (permissions, device busy), use a different product ID to create a distinct device.

### Slot Already Active When Finger Down Requested

**Problem:** Node A lifts its finger, but before the `SYN_REPORT` is processed, Node B tries to use the same slot.

**Solution:** Impossible by design — each node has a fixed slot assignment in the profile. Two nodes never share a slot. The engine validates this at profile load time.

### Rapid Tap Events

**Problem:** User spams a tap key very fast (30+ presses/sec). Each press/release generates a TouchDown/TouchUp pair. Some games can't register this many distinct touches.

**Solution:** Add a minimum hold time for `tap` nodes. Default: 30ms between TouchDown and TouchUp. If the key is released before 30ms, delay the TouchUp. Configurable via `min_hold_ms` field on tap nodes.

---

## Coordinate Edge Cases

### Clamping

**Problem:** Computed touch position falls outside screen bounds (e.g., joystick offset + radius exceeds 1.0).

**Solution:** Clamp every computed coordinate to [0.0, 1.0] before converting to pixels. Then clamp pixel coordinates to [0, screen_width-1] and [0, screen_height-1]. Double-clamping ensures no out-of-bounds values reach the kernel.

### Zero-Size Region

**Problem:** `mouse_camera` region has `w: 0` or `h: 0`. Camera finger has nowhere to move.

**Solution:** Validation rejects regions with zero or negative dimensions at profile load time.

### Resolution Mismatch Between Profile and Screen

**Problem:** Profile was created on 1920x1080, running on 2560x1440. Relative coordinates still work, but the `radius` field on joystick nodes was tuned for the original resolution.

**Solution:** Relative coordinates handle this automatically. `radius: 0.07` means 7% of the screen regardless of resolution. This is a feature, not a bug.

---

## System Edge Cases

### Daemon Crash (SIGKILL)

**Problem:** Daemon is killed without cleanup. uinput device persists with no active touches. evdev grabs are released by the kernel on process death.

**Solution:** On next startup:
1. Check for orphaned "Phantom Virtual Touch" devices
2. Attempt to destroy them
3. Create fresh device
4. Normal startup continues

### Daemon Already Running

**Problem:** User starts a second daemon instance.

**Solution:** On startup, attempt to connect to the Unix socket. If connection succeeds, another daemon is running — print error and exit. If connection fails (ECONNREFUSED), the socket is stale — delete it and proceed.

### No Input Devices Found

**Problem:** No keyboard or mouse matches the capability filter.

**Solution:** Exit with clear error: "No input devices found. Check permissions on /dev/input/ (need root or input group membership)."

### Permission Denied on /dev/uinput

**Problem:** Non-root user tries to create uinput device.

**Solution:** Exit with clear error: "Permission denied on /dev/uinput. Run as root or add user to 'input' group and set udev rules."

### Profile File Locked / Unreadable

**Problem:** Profile JSON is being written by GUI at the same time daemon tries to read it.

**Solution:** Read the entire file into a buffer first, then parse. If the file is partially written (invalid JSON), return a clear error. No file locking needed — atomic read is sufficient.

### Signal Handling (SIGTERM, SIGINT)

**Problem:** User sends Ctrl+C or system shuts down. Need clean teardown.

**Solution:** Install signal handlers for `SIGTERM` and `SIGINT`. Signal handler sets an atomic flag. Main loop checks the flag each iteration, breaks if set. Cleanup code runs: lift all fingers → destroy uinput device → release evdev grabs → delete Unix socket → exit.

---

## GUI Edge Cases

### Screenshot Not Available

**Problem:** User hasn't taken a screenshot or `adb` isn't available. Canvas has no background.

**Solution:** Render a dark gray background with a text overlay: "Drop a screenshot here or press Ctrl+O to load". GUI works fine without a screenshot — nodes are still visible and draggable.

### Profile File Doesn't Exist Yet

**Problem:** User opens GUI but hasn't created a profile.

**Solution:** Start with an empty profile. "Save" prompts for a filename. Default location: `~/.config/phantom/profiles/untitled.json`.

### Window Closed While Editing

**Problem:** User closes GUI window without saving.

**Solution:** Track dirty state. On close, if dirty, show confirmation dialog. Auto-save to a temporary file (`~/.config/phantom/profiles/.autosave.json`) as backup.

---

## IPC Edge Cases

### Socket Stale After Crash

**Problem:** Daemon crashed, Unix socket file still exists. New daemon can't bind.

**Solution:** On bind failure, check if socket is connectable. If not, delete stale socket and retry bind. This is handled in daemon startup code.

### Multiple GUI Instances

**Problem:** User opens multiple GUI windows.

**Solution:** Fine. Each GUI connects to the same daemon socket, sends a command, gets a response, disconnects. Multiple concurrent connections are supported.

### Large Profile

**Problem:** Profile with many nodes generates a large JSON file (>64KB).

**Solution:** IPC request size limit is 64KB. This is sufficient for 100+ nodes. If a profile somehow exceeds this, the daemon rejects the request with "profile too large" error.
