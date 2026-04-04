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

For raw low-level device tracing only when needed:

```bash
sudo env PHANTOM_TRACE_DETAIL=1 phantom --trace --daemon
```

If `sudo phantom` is not found after install, rerun `./install.sh`. The installer now places a sudo-visible `phantom` launcher in `/usr/local/bin` when possible.

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

For recommended profile structures for shooters, layered contexts, sprint-lock drags, and large game layouts, see [GAME_PATTERNS.md](GAME_PATTERNS.md).

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
- if you want to refresh shipped examples or the generated config in place, use `./install.sh -o`

## Runtime Hotkeys

Configured in:

- `~/.config/phantom/config.toml`
- `[runtime_hotkeys]`

Defaults:

- `F1` -> toggle mouse routing
- `F8` -> toggle capture
- `F9` -> toggle pause
- `F10` -> toggle the experimental debug overlay preview
- `F2` -> shutdown daemon

Fn row warning:

- on many keyboards the function row only emits true `F1`, `F8`, and `F10` events when Fn Lock is enabled
- if `F2` works but `F1`, `F8`, or `F10` appear dead, check Fn Lock first

Overlay notes:

- `F10` shows or hides an experimental host-side debug preview of the current profile
- it is not intended for normal gameplay
- it may behave differently across compositors and sessions
- overlay launcher output is written to `~/.config/phantom/overlay.log`

## Tracing Levels

Use:

- `phantom --daemon` for normal operation
- `phantom --trace --daemon` for useful runtime diagnosis
- `PHANTOM_TRACE_DETAIL=1 phantom --trace --daemon` only for raw input/device forensics

The detail flag exists because the raw evdev path is intentionally much noisier than normal trace logging.

## Aim Operations

`aim` has three modes.

Runtime note:

- entering capture puts Phantom into owned menu-touch mode by default
- `F1` switches between gameplay aim and owned menu-touch
- it no longer destroys toggle-look state
- `while_held` mouse buttons are resynced when mouse routing is turned back on
- status output now includes:
  - the active menu-touch backend
  - the current runtime mouse mode

## Menu Touch Operations

When capture is active and mouse mode is `menu_touch`, Phantom routes:

- left click -> touch down / up
- mouse drag -> touch move
- a small owned cursor overlay shows where those touches will land

Backend behavior:

- Phantom keeps the physical mouse grabbed during capture
- when Phantom enters menu-touch, it seeds its internal cursor from host cursor position when possible
- Hyprland prefers compositor-native cursor/client geometry for that seed
- X11/XWayland sessions fall back to exact visible-cursor helper mapping for that seed
- if no exact host seed is available, Phantom reuses its existing internal cursor position
- after the initial seed, menu-touch uses the Phantom-owned cursor directly and no longer depends on desktop window activation semantics
- while menu-touch is active, Phantom shows a dedicated cursor overlay instead of moving the desktop cursor
- on Wayland sessions, that cursor overlay is provided through a layer-shell surface with input passthrough
- on touchpads, Phantom now synthesizes tap-to-click and double-tap-hold drag locally because those gestures are no longer provided by the desktop once Phantom owns the mouse

### `always_on`

Use when:

- capture should always steer the camera

### `while_held`

Use when:

- one key should temporarily enable look mode
- right mouse button should both ADS and enable aim

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

Practical difference:

- `fixed` is for visible static sticks and now uses a staged engage path
- `floating` is for drag zones and starts moving immediately from a runtime origin

### `drag`

Use for:

- swipe games like Temple Run and Subway Surfers
- sprint-lock drags in PUBG-style movement systems
- one-shot directional gestures

## Suggested Game Workflows

### PUBG Mobile

Use:

- `pubg.json` for the main richer starter based on a real custom-layout screen
- `pubg-small.json` for a compact baseline

Typical mapping model:

- `WASD` -> movement joystick
- `MouseLeft` -> fire
- `MouseRight` -> ADS and/or `aim`
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
