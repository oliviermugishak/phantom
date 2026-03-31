# Architecture

Phantom has five runtime pieces:

1. evdev capture
2. keymap engine
3. uinput touch injector
4. IPC control plane
5. native mapper GUI

## Data Flow

```text
/dev/input/event* -> Phantom input capture -> keymap engine -> /dev/uinput -> Waydroid
                                                ^                   ^
                                                |                   |
                                            profile data        touch events
                                                ^
                                                |
                                      CLI / GUI / IPC requests
```

The GUI is still separate from the injector, but it is no longer disk-only. It can push live profiles and runtime commands over IPC.

## Input Capture

Phantom scans `/dev/input/event*` on startup, classifies devices by capabilities, reopens matching keyboard and mouse devices in nonblocking mode, and applies `EVIOCGRAB`.

Current behavior:

- exclusive grab on startup
- edge-triggered `epoll`
- key repeat filtered out
- `SYN_DROPPED` buffered events discarded until the next `SYN_REPORT`
- runtime grab control through IPC and `F8`
- no hotplug rescan for new devices

Tradeoff:

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

## Touch Injection

Phantom creates a virtual direct-touch device through `/dev/uinput`.

Key properties:

- event types: `EV_ABS`, `EV_KEY`, `EV_SYN`
- axes: `ABS_MT_SLOT`, `ABS_MT_TRACKING_ID`, `ABS_MT_POSITION_X`, `ABS_MT_POSITION_Y`
- key bit: `BTN_TOUCH`
- property bit: `INPUT_PROP_DIRECT`
- identity: `BUS_VIRTUAL`, stable vendor/product IDs, `"Phantom Virtual Touch"`

Runtime touch model:

- 10 slots, `0..9`
- tracking ID equals slot number
- `BTN_TOUCH` asserted on first touch and cleared on last release

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
