# Phantom — Architecture

## Scope Contract

Phantom does **one thing**: map keyboard + mouse input to virtual multitouch events that Waydroid sees as real hardware. That's it.

**Assumptions we enforce:**
- Waydroid is running fullscreen on the primary display
- One display, one resolution, no rotation detection
- User runs Phantom on the same Linux kernel as Waydroid (LXC container)
- Hyprland compositor (wlroots-based)
- Root access available for `/dev/input` and `/dev/uinput`

**What Phantom does NOT do:**
- Multi-monitor management
- Screen rotation detection
- Waydroid lifecycle management
- ADB interaction of any kind
- Game-specific hacks beyond the node system

---

## System Diagram

```
┌─────────────────────────────────────────────────────────┐
│ Linux Host — Hyprland compositor                        │
│                                                         │
│  /dev/input/event*  ──evdev EVIOCGRAB──►  Phantom      │
│  (keyboard, mouse)                         daemon       │
│                                                 │       │
│                                                 ▼       │
│                                          keymap engine  │
│                                          (state machine)│
│                                                 │       │
│                                                 ▼       │
│  /dev/uinput  ◄─── ioctl write ───── touch injector    │
│  (virtual touchscreen)              MT Protocol B      │
│                                                 │       │
│                                                 ▼       │
│                                          Waydroid (LXC) │
│                                          sees real HW   │
│                                                 │       │
│                                                 ▼       │
│                                          Android app    │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│ phantom-gui (separate process, optional)                │
│ egui + winit — regular floating window                  │
│ communicates with daemon over Unix socket               │
│                                                         │
│  canvas: screenshot background + draggable nodes        │
└─────────────────────────────────────────────────────────┘
```

---

## Component 1: Input Capture

### Mechanism

Open each `/dev/input/event*` device. Query capabilities via `EVIOCGID` and `EVIOCGBIT`. Filter for:
- Devices with `EV_KEY` capability AND `KEY_A` through `KEY_Z` present → keyboard
- Devices with `EV_REL` capability AND `REL_X` + `REL_Y` present → mouse

Apply `EVIOCGRAB` ioctl to exclusively capture each matching device. This prevents any events from reaching the compositor or other applications.

### Why evdev grab, not Wayland protocols

`zwp_relative_pointer_v1` and `zwp_pointer_constraints_v1` require the compositor to still process mouse events. But if we're grabbing via evdev, the compositor never sees the events. These two approaches are **mutually exclusive**.

We pick evdev grab because:
- Zero compositor dependency for input
- Works even if Hyprland updates break protocol support
- Direct kernel-level capture — no event loop round-trips
- We compute mouse deltas ourselves from raw `EV_REL` events

### Event Flow

```
kernel event queue
       │
       ▼
  read() on fd (non-blocking via epoll/tokio)
       │
       ▼
  evdev::RawEvent { type, code, value }
       │
       ▼
  filter: ignore SYN_DROPPED, ignore repeats
       │
       ▼
  dispatch to keymap engine
```

### Device Discovery

On startup:
1. Enumerate `/dev/input/event*` via `std::fs::read_dir`
2. For each device, open read-only, query capabilities
3. Classify as keyboard or mouse
4. Re-open with write mode, apply `EVIOCGRAB`
5. Store fd in epoll/tokio reactor

If a device disappears (unplug), epoll returns `ENODEV`. Remove from watch list. **No hotplug re-scan** — if user plugs in a new device, restart the daemon. This is a deliberate simplicity choice.

---

## Component 2: Keymap Engine

### Design

Pure state machine. No async, no I/O, no side effects. Takes `InputEvent` in, produces `Vec<TouchCommand>` out.

```
InputEvent                    TouchCommand
  ├─ KeyPress(key)      →     ├─ TouchDown { slot, x, y }
  ├─ KeyRelease(key)    →     ├─ TouchMove { slot, x, y }
  ├─ MouseMove(dx, dy)  →     └─ TouchUp   { slot }
  └─ MouseButton(btn, state)
```

### State Tracking

The engine maintains:
- `key_states: HashMap<Key, bool>` — which keys are currently pressed
- `node_states: HashMap<NodeId, NodeState>` — per-node internal state
- `active_slots: [Option<TrackingId>; 10]` — which slots have fingers down

Each node type has its own state struct:

```rust
struct JoystickState {
    pressed: [bool; 4],     // up, down, left, right
    finger_active: bool,
}

struct MouseCamState {
    finger_active: bool,
    last_x: f64,
    last_y: f64,
}

struct HoldTapState {
    held: bool,
}

struct TapState {
    // stateless — just fires on key edge
}

struct RepeatTapState {
    active: bool,
    last_tick: Instant,
}
```

### Coordinate System

All positions stored as **relative coordinates** (0.0 to 1.0). At runtime, multiply by screen resolution:

```
pixel_x = (rel_x * screen_width) as i32
pixel_y = (rel_y * screen_height) as i32
```

Screen resolution read once at daemon startup from `/sys/class/graphics/fb0/virtual_size` or via `ioctl(FBIOGET_VSCREENINFO)` on `/dev/fb0`. Fallback: read from Waydroid config or use a hardcoded default (1920x1080) with a warning.

### Key Repeat Filtering

Linux input subsystem sends `EV_MSC MSC_SCAN` and repeat `EV_KEY` events when a key is held. The engine ignores any `EV_KEY` event where the value is `2` (repeat). Only `1` (press) and `0` (release) are processed.

### Node Types

See [PROFILES.md](./PROFILES.md) for full specification of each node type.

---

## Component 3: Touch Injector

### Device Creation

Open `/dev/uinput` with `O_WRONLY | O_NONBLOCK`. Issue the following ioctls in sequence:

```
UI_SET_EVBIT   → EV_ABS, EV_KEY, EV_SYN
UI_SET_ABSBIT  → ABS_MT_SLOT, ABS_MT_TRACKING_ID, ABS_MT_POSITION_X, ABS_MT_POSITION_Y
UI_SET_KEYBIT  → BTN_TOUCH

// Configure absolute axis ranges
struct uinput_abs_setup {
    code: ABS_MT_SLOT,
    absinfo: { value: 0, minimum: 0, maximum: 9, fuzz: 0, flat: 0, resolution: 0 }
}
// same for ABS_MT_POSITION_X (0..screen_width)
// same for ABS_MT_POSITION_Y (0..screen_height)

// Set device properties
UI_SET_PROPBIT → INPUT_PROP_DIRECT   // direct touch, not indirect

// Configure device identity
struct uinput_setup {
    name: "Phantom Virtual Touch",
    id: { bustype: BUS_VIRTUAL, vendor: 0x1234, product: 0x5678, version: 1 }
}

UI_DEV_CREATE
```

### Event Sequences

Every touch action is a structured sequence ending with `SYN_REPORT`:

**Finger down at slot 3, position (500, 800):**
```
write: EV_ABS  ABS_MT_SLOT        3
write: EV_ABS  ABS_MT_TRACKING_ID 3        // non-negative = finger down
write: EV_ABS  ABS_MT_POSITION_X  500
write: EV_ABS  ABS_MT_POSITION_Y  800
write: EV_SYN  SYN_REPORT          0
```

**Finger move slot 3 to (520, 790):**
```
write: EV_ABS  ABS_MT_SLOT        3
write: EV_ABS  ABS_MT_POSITION_X  520
write: EV_ABS  ABS_MT_POSITION_Y  790
write: EV_SYN  SYN_REPORT          0
```

**Finger up slot 3:**
```
write: EV_ABS  ABS_MT_SLOT        3
write: EV_ABS  ABS_MT_TRACKING_ID -1       // negative = finger lifted
write: EV_SYN  SYN_REPORT          0
```

### Slot Allocation

Each node is assigned a fixed slot (0–9) in the profile JSON. The engine never reassigns slots at runtime. This guarantees:
- Node A's touch never interferes with node B's touch
- Up to 10 simultaneous touches (standard MT limit)
- Predictable behavior for debugging

### Tracking ID Strategy

Use the slot number as the tracking ID. Slot 0 gets tracking ID 0, slot 3 gets tracking ID 3. This is valid — tracking IDs just need to be unique per active finger, not sequential.

### Device Teardown

On daemon shutdown (SIGTERM, SIGINT, or normal exit):
1. For each active slot: send finger-up event
2. Wait 10ms for kernel to flush
3. Call `UI_DEV_DESTROY` ioctl
4. Close `/dev/uinput` fd

If the daemon is SIGKILL'd, the uinput device persists in the kernel but with no active touches. On next startup, the daemon creates a new device (different product ID or version number to avoid confusion). Old orphaned devices can be cleaned via `evemu-detach` or reboot.

---

## Component 4: IPC

### Transport

Unix domain socket at `$XDG_RUNTIME_DIR/phantom.sock` (typically `/run/user/1000/phantom.sock`).

### Protocol

Newline-delimited JSON. One JSON object per line. Connectionless-style: each message is self-contained.

**Client → Daemon:**
```json
{"cmd": "load_profile", "path": "~/.config/phantom/profiles/pubg.json"}
{"cmd": "reload"}
{"cmd": "status"}
{"cmd": "set_sensitivity", "value": 1.5}
{"cmd": "pause"}
{"cmd": "resume"}
```

**Daemon → Client:**
```json
{"ok": true, "message": "profile loaded"}
{"ok": false, "error": "file not found"}
{"status": "running", "profile": "PUBG Mobile", "active_slots": [0, 1, 2]}
```

### Security

Socket permissions: `0600` (owner read/write only). No authentication beyond filesystem permissions.

---

## Component 5: GUI (Separate Binary)

### Why Separate

- Daemon needs root for `/dev/input` and `/dev/uinput`
- GUI should run as regular user
- Daemon can run headless forever, GUI opened only for profile editing
- Crash in GUI doesn't kill daemon

### Window

Regular `winit` window with `egui` rendering via `eframe`. No layer shell. No overlay. Just a normal application window with:
- Screenshot background (loaded from file, taken via external `adb exec-out screencap` or `grim`)
- Canvas with pan/zoom
- Draggable node markers
- Node property panel (sidebar)
- Save/Load buttons

### Coordinate Mapping

Canvas displays the screenshot at the screen's native resolution (or scaled to fit window). Node positions are stored as relative coordinates (0.0–1.0) regardless of canvas zoom level.

When user drags a node to pixel (450, 720) on a 1920x1080 canvas:
```
rel_x = 450.0 / 1920.0 = 0.234
rel_y = 720.0 / 1080.0 = 0.667
```

---

## Dependencies

```toml
[dependencies]
tokio      = { version = "1", features = ["full"] }
nix        = { version = "0.27", features = ["ioctl", "fs", "signal"] }
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow     = "1"
tracing    = "0.1"
tracing-subscriber = "0.3"

# GUI binary only
eframe     = "0.28"
egui       = "0.28"
image      = "0.25"
```

### Why no `evdev` or `uinput` crates

Both are either unmaintained or too high-level. We need precise control over:
- ioctl argument ordering
- Device capability flags
- Error handling per ioctl
- Non-standard ABS axis configuration

Raw `nix::ioctl_*` macros give us this with zero abstraction overhead. The actual ioctl calls are ~30 lines of code — not worth a dependency.

---

## File Layout

```
phantom/
├── Cargo.toml
├── docs/
│   ├── ARCHITECTURE.md    ← this file
│   ├── PROTOCOL.md        ← uinput MT protocol details
│   ├── PROFILES.md        ← JSON profile format
│   ├── IPC.md             ← daemon communication
│   ├── EDGE_CASES.md      ← every edge case and solution
│   └── BUILD.md           ← build phases
├── src/
│   ├── main.rs            ← daemon entry point
│   ├── input.rs           ← evdev capture, device discovery
│   ├── engine.rs          ← keymap state machine
│   ├── inject.rs          ← uinput device creation + event injection
│   ├── ipc.rs             ← Unix socket server
│   ├── profile.rs         ← JSON profile parsing + validation
│   └── error.rs           ← error types
├── phantom-gui/
│   ├── src/
│   │   └── main.rs        ← GUI entry point
│   └── Cargo.toml
└── profiles/
    └── pubg.json          ← example profile
```

---

## Runtime Flow

```
1. phantom daemon starts
2. Read screen resolution from /sys/class/graphics/fb0
3. Create uinput virtual touchscreen device
4. Discover and grab input devices
5. Load default profile from ~/.config/phantom/profiles/
6. Enter event loop:
   a. epoll waits for input events
   b. Parse evdev event
   c. Feed to keymap engine
   d. Engine produces TouchCommands
   e. Write TouchCommands to uinput fd
   f. Handle IPC messages (profile reload, status queries)
7. On shutdown: release all slots, destroy uinput device, release evdev grabs
```

---

## Design Principles

1. **No magic.** Every ioctl, every event sequence is explicit and documented.
2. **No heuristics.** No auto-detection of "which game is running." User loads a profile, that's it.
3. **No daemon communication with Waydroid.** We write to uinput. Kernel handles the rest.
4. **Crash-safe by construction.** Signal handlers release all resources. Orphaned state is detectable.
5. **Profile is the config.** No separate settings file beyond a minimal `config.toml` for global defaults.
