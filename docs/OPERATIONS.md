# Operations

This document is the day-to-day runtime guide for Phantom.

It assumes the project is already built and configured. If not, start with [INSTALL.md](INSTALL.md).

## Runtime Model

Phantom runtime behavior is controlled by four independent concerns:

1. daemon process
2. capture state
3. mouse routing state
4. engine pause state

These are separate on purpose.

### Daemon Running

When the daemon is running:

- Phantom reads keyboard and mouse devices
- the current profile engine exists in memory
- the IPC socket is available to the CLI and GUI

### Capture Enabled

When capture is enabled:

- Phantom grabs the input devices for gameplay
- gameplay events are allowed to reach the engine

When capture is disabled:

- gameplay events are not forwarded into the engine
- desktop interaction returns

### Mouse Routed

When mouse routing is enabled:

- mouse movement can drive `mouse_camera`
- mouse buttons and wheel events bound in profiles are forwarded into the engine

When mouse routing is disabled:

- mouse-originated gameplay events are suppressed
- active mouse-driven touches are released
- capture may remain active for keyboard-driven gameplay
- the daemon still keeps the physical keyboard grabbed so runtime hotkeys continue to work
- outside capture, non-hotkey keyboard events are replayed back to Linux through Phantom's virtual desktop keyboard

This is the key distinction that makes menu interaction and hybrid workflows possible.

### Engine Paused

When paused:

- Phantom still runs
- capture can still exist
- the engine stops producing new touch commands

Use pause when you want the daemon alive but injection frozen.

## Recommended Startup Flow

For the current recommended backend:

```bash
waydroid session start
waydroid show-full-ui
sudo waydroid status
sudo phantom --trace --daemon
```

Before starting Phantom, confirm:

- `Session: RUNNING`
- `Container: RUNNING`

If the container is `FROZEN`, open the UI or the game first.

## CLI Control Surface

Common commands:

```bash
phantom version
phantom status
phantom audit <profile.json>
phantom load <profile.json>
phantom reload
phantom enter-capture
phantom exit-capture
phantom toggle-capture
phantom grab-mouse
phantom release-mouse
phantom toggle-mouse
phantom pause
phantom resume
phantom sensitivity <value>
phantom shutdown
```

## Studio Runtime Surface

`phantom-studio` is now more than a profile editor. Its `Runtime` tab can:

- request daemon launch
- show daemon connection state
- push the current profile live
- enter or leave capture
- toggle mouse routing
- shut the daemon down

If your system still requires elevated input access, the studio will prefer `pkexec` when it is available. Otherwise, start `phantom --daemon` manually and return to the studio.

## Runtime Hotkeys

Configured in:

- `~/.config/phantom/config.toml`
- `[runtime_hotkeys]`

Default bindings:

- `F8` -> toggle capture
- `F1` -> toggle mouse routing
- `F9` -> toggle pause
- `F2` -> shutdown daemon

Use `""` or `"none"` to disable any of them.

If `F2` works but `F1` or `F8` do not, check Fn Lock first. On many keyboards the top row only sends real function keys when Fn Lock is enabled.

## GUI Workflow

Typical editor workflow:

1. open or create a profile
2. confirm the `screen` contract
3. load a screenshot if useful
4. place controls
5. bind real input
6. tune positions, regions, and sensitivity
7. `Push Live`
8. enter capture and test

Useful GUI runtime actions:

- `Push Live`
- `Pause`
- `Resume`
- `Enter Capture`
- `Exit Capture`
- `Toggle Capture`
- `Grab Mouse`
- `Release Mouse`

## Mouse Look Operations

`mouse_camera` has three operational modes.

### `always_on`

Meaning:

- once capture is active and mouse routing is enabled, mouse movement always drives the look region

Best for:

- games where camera motion should always follow the mouse during capture

### `while_held`

Meaning:

- the activation key enables mouse look while it is held
- releasing the key disables mouse look and lifts the synthetic finger immediately

Best for:

- aim or scoped camera flows
- situations where a key should both hold another touch and enable look

### `toggle`

Meaning:

- pressing the activation key toggles mouse look on or off

Best for:

- mode-based games
- players who want explicit camera mode switching

## Recommended PUBG Workflow

Start with this mental model:

- `WASD` -> joystick
- `MouseLeft` -> fire
- `MouseRight` -> either ADS only, or ADS plus mouse-look activation

Two reasonable setups:

### Setup A: `while_held` on `MouseRight`

Use this if right-click should:

- hold ADS
- enable look at the same time

This is a valid configuration because Phantom can bind the same physical key to:

- one `hold_tap` node
- one `mouse_camera` activation key

The shipped `profiles/pubg.json` now uses this setup by default.

### Setup B: `toggle` on a spare key

Use this if you want:

- ADS on one key
- camera mode on another key

This is often easier to debug and tune first.

## Logs And Health Signals

Important places to inspect:

- daemon terminal output
- `phantom status`
- Waydroid status
- container server log: `/data/local/tmp/phantom-server.log`

For `android_socket`, the important health signals are:

- daemon says it connected to the Android touch server
- Waydroid container is `RUNNING`
- the server log shows a client connection

For `uinput`, the important health signals are:

- Android sees the virtual touchscreen device
- Waydroid reacts to injected touches from that device

## Shutdown

Graceful shutdown paths:

- `phantom shutdown`
- configured shutdown hotkey

If you are debugging and want a clean reset:

1. stop Phantom
2. stop or restart Waydroid if necessary
3. start Waydroid again
4. start Phantom again
