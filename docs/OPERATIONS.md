# Operations

This is the day-to-day runtime guide for Phantom.

If the project is not installed yet, start with [INSTALL.md](INSTALL.md).

## Runtime Model

Phantom runtime behavior is easiest to reason about as four independent controls:

1. daemon running
2. capture enabled
3. mouse routing enabled
4. engine paused

These are separate deliberately.

Why:

- the daemon can stay alive while profiles change
- capture controls whether gameplay input should flow into the engine
- mouse routing controls whether mouse-originated gameplay events should reach the game
- pause freezes touch output without requiring a daemon restart

## Recommended Startup

```bash
waydroid session start
waydroid show-full-ui
sudo waydroid status
sudo phantom --trace --daemon
```

Required Waydroid state:

- `Session: RUNNING`
- `Container: RUNNING`

If the container is `FROZEN`, the Android backend can appear partially alive while still failing readiness checks.

## Daily CLI Commands

```bash
phantom --version
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
phantom list
phantom shutdown
```

## GUI Workflow

Start the editor with:

```bash
phantom-gui
```

The GUI is a mapping editor and runtime control surface.

Typical workflow:

1. open or create a profile
2. confirm the screen contract
3. place controls
4. bind real keys or mouse buttons
5. save into `~/.config/phantom/profiles/`
6. `Push Live`
7. enter capture
8. test in the game

Runtime actions available in the GUI:

- start daemon
- shutdown daemon
- push live
- enter capture
- exit capture
- toggle capture
- grab mouse
- release mouse

## Profile Library Behavior

The GUI discovers profiles from:

- `~/.config/phantom/profiles/`

It does not read the repository `profiles/` directory directly.

The supported sync flow is:

1. keep shipped starter profiles in the repository `profiles/` directory
2. seed missing ones into `~/.config/phantom/profiles/` through `./install.sh`
3. let the GUI load and save against the user profile library

Practical rule:

- if a new shipped profile does not appear in the GUI, rerun `./install.sh`

## Runtime Hotkeys

Configured in:

- `~/.config/phantom/config.toml`
- `[runtime_hotkeys]`

Defaults:

- `F1` -> toggle mouse routing
- `F8` -> toggle capture
- `F9` -> toggle pause
- `F10` -> toggle the transparent control preview overlay
- `F2` -> shutdown daemon

Fn row warning:

- on many keyboards the function row only emits true `F1`, `F8`, and `F10` events when Fn Lock is enabled
- if `F2` works but `F1`, `F8`, or `F10` appear dead, check Fn Lock first

Overlay notes:

- `F10` shows or hides a transparent click-through preview of the current profile
- the overlay is visual only and does not intercept desktop or game input

## Mouse Look Operations

`mouse_camera` has three modes.

### `always_on`

Use when:

- capture should always steer the camera

### `while_held`

Use when:

- one key should temporarily enable look mode
- right mouse button should both ADS and enable camera look

### `toggle`

Use when:

- you want explicit on/off camera mode switching

## Joystick And Drag Operations

### `joystick`

Use for:

- continuous movement
- visible sticks
- floating movement zones

Modes:

- `fixed`
- `floating`

### `drag`

Use for:

- swipe games like Temple Run and Subway Surfers
- sprint-lock drags in PUBG-style movement systems
- one-shot directional gestures

## Suggested Game Workflows

### PUBG Mobile

Use:

- `pubg.json` for a compact baseline
- `pubg-mobile-layout1.json` for a richer starter based on a real custom-layout screen

Typical mapping model:

- `WASD` -> movement joystick
- `MouseLeft` -> fire
- `MouseRight` -> ADS and/or `mouse_camera`
- `LeftShift` -> sprint-lock drag

### Temple Run / Subway Surfers

Use:

- `temple-run.json`
- `subway-surfers.json`

Typical mapping model:

- `A` -> swipe left
- `D` -> swipe right
- `W` -> swipe up
- `S` -> swipe down

### Asphalt 8 / Asphalt 9

Use:

- `asphalt8.json`
- `asphalt9.json`

These are starter keyboard layouts for tap/hold driving controls, not full analog steering-wheel emulation.

## What Not To Expect

Phantom does not currently inject sensors.

So:

- tilt controls are not supported
- accelerometer-based coin collection in Temple Run is not supported

If a game exposes a touch-based alternative, use that.
