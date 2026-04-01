# Architecture

Phantom has five runtime pieces:

1. evdev capture
2. keymap engine
3. touch injector backend
4. IPC control plane
5. native mapper GUI

## Data Flow

```text
/dev/input/event* -> Phantom input capture -> keymap engine -> injector backend
                                                ^                    ^
                                                |                    |
                                            profile data         touch events
                                                ^
                                                |
                                      CLI / GUI / IPC requests

uinput path:
  injector backend -> /dev/uinput -> Waydroid kernel input bridge

android_socket path:
  injector backend -> TCP -> app_process server -> InputManager.injectInputEvent()
```

The GUI is still separate from the injector, but it is no longer disk-only. It can push live profiles and runtime commands over IPC.

For static inspection, the CLI also exposes `phantom audit <profile.json>`, which renders the slot map directly from the profile model without starting the daemon.

## Input Capture

Phantom scans `/dev/input/event*` on startup, classifies devices by capabilities, and reopens matching keyboard and mouse devices in nonblocking mode.

Current behavior:

- shared evdev reads on startup
- edge-triggered `epoll`
- key repeat filtered out
- `SYN_DROPPED` buffered events discarded until the next `SYN_REPORT`, then key state is resynced with `EVIOCGKEY`
- runtime grab control through IPC and `F8`
- optional mouse-only release through `F1` while capture is active
- no hotplug rescan for new devices

Tradeoff:

- hotkeys can still be observed before capture is enabled
- compositor-independent on Wayland or X11
- when capture is on, the grabbed devices do not control the desktop

## Keymap Engine

The engine is a synchronous state machine.

Inputs:

- key press
- key release
- mouse movement

Outputs:

- `TouchDown`
- `TouchMove`
- `TouchUp`

Supported node types:

- `tap`
- `hold_tap`
- `toggle_tap`
- `joystick`
- `mouse_camera`
- `repeat_tap`
- `macro`
- `layer_shift`

Current timing:

- input polling every 4 ms
- timer-driven nodes every 16 ms

## Touch Injection Backends

Phantom currently supports two injector targets behind one interface.

### `uinput`

Phantom creates a virtual direct-touch device through `/dev/uinput`.

Key properties:

- event types: `EV_ABS`, `EV_KEY`, `EV_SYN`
- axes: `ABS_X`, `ABS_Y`, `ABS_PRESSURE`, `ABS_MT_TOUCH_MAJOR`, `ABS_MT_SLOT`, `ABS_MT_TRACKING_ID`, `ABS_MT_POSITION_X`, `ABS_MT_POSITION_Y`, `ABS_MT_PRESSURE`
- key bits: `BTN_TOUCH`, `BTN_TOOL_FINGER`, `BTN_TOOL_DOUBLETAP`, `BTN_TOOL_TRIPLETAP`, `BTN_TOOL_QUADTAP`
- property bit: `INPUT_PROP_DIRECT`
- identity: `BUS_VIRTUAL`, stable vendor/product IDs, `"Phantom Virtual Touch"`

Runtime touch model:

- 10 slots, `0..9`
- tracking IDs are monotonic and independent from slot numbers
- pointer-emulation state is updated alongside MT slot state
- touch commands are batched into a single `SYN_REPORT`
- safest startup order is Phantom first, then Waydroid session start or restart

### `android_socket`

Phantom can also send touch commands to a small `app_process` server inside Waydroid over TCP.

Current shape:

- host daemon still owns evdev capture and keymap logic
- Rust backend serializes `TouchDown`, `TouchMove`, and `TouchUp`
- Android server reconstructs `MotionEvent`s and injects them through `InputManager.injectInputEvent()`
- the daemon discovers the Waydroid container IP from `waydroid status`
- the daemon stages `phantom-server.jar` into `/data/local/tmp/` through `waydroid shell` stdin
- the Android server binds `0.0.0.0:27183` by default
- `uinput` remains available as a fallback backend
- Waydroid session must already be running before the daemon starts
- the daemon launches the server through `waydroid shell`
- the server is a single-client process today; if it dies, restart the daemon

Health signals for this path:

- daemon log shows `backend=android_socket`
- daemon log shows successful server connection
- server log inside Waydroid shows client connected
- container port `27183` is listening

Signals that do not apply here:

- `getevent` showing `Phantom Virtual Touch`
- `dumpsys input` showing a new kernel-level Phantom device
- IDC classification effects

## Resolution Handling

Phantom now uses a strict fullscreen contract.

Resolution source of truth:

1. `[screen]` in `config.toml`
2. `screen` in the default profile

If neither exists, daemon startup fails. Phantom no longer silently guesses from host framebuffer state.

## IPC

IPC is newline-delimited JSON over a Unix socket.

It is used for:

- loading profiles from disk
- pushing in-memory profiles live
- querying runtime status
- pause and resume
- capture on and off
- sensitivity changes
- shutdown

## GUI

`phantom-gui` is a native `eframe` / `egui` mapper.

Current capabilities:

- screenshot-first canvas
- point placement tools
- region resize handles
- live key capture
- inline rename
- macro editing
- layer switch editing
- daemon status and live push

## Operational Limits

Phantom is still intentionally narrow:

- no hotplug rescan
- no compositor protocol integration
- no automatic floating joystick discovery
- no multi-monitor or rotation management

Best fit:

- one known fullscreen Waydroid surface
- one configured touch resolution
- manually tuned profiles
