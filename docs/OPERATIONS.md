# Operations

Day-to-day use after install. First-time setup is in [INSTALL.md](INSTALL.md).
When something is broken, start with [TROUBLESHOOT.md](TROUBLESHOOT.md).

## What are the four switches I actually control?

1. Is the daemon running?
2. Is capture on?
3. Is the owned mouse in aim or menu-touch?
4. Is the engine paused?

They are independent so you can reload a profile without restarting, leave
capture to use the desktop, or freeze touches without dropping grabs.

## How do I start a normal session?

From the graphical session:

```bash
waydroid session start
waydroid show-full-ui
sudo waydroid status
sudo -E phantom --daemon
```

Then, as your desktop user, never with sudo:

```bash
phantom-gui
```

Add `--trace` to the daemon when you need lifecycle logs. Use
`PHANTOM_TRACE_DETAIL=1` only for raw evdev forensics.

If `sudo phantom` is not found, rerun `./install.sh` so the
`/usr/local/bin` wrapper exists. Do not `systemctl enable` the system unit.
After udev + group `input`, `/usr/lib/systemd/user/phantom.service` is the
optional systemd path.

If android auto-launch fails because `android.server_jar` points to an old
source path, Phantom now falls back to the installed jar in
`~/.local/share/phantom/android/`, then to `../lib/phantom/` relative to the
running binary, then to `/usr/lib/phantom/`, and finally to a built jar in the
current source tree before failing.

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
- switch to aim
- switch to menu-touch

## Profile Library Behavior

The GUI discovers profiles from:

- `~/.config/phantom/profiles/`

It does not read the repository `profiles/` directory directly.

Shipped starters live in the repo or in `/usr/share/phantom/profiles/`. The
GUI copies missing files into the user library on startup, and Settings can
repeat that seed. `./install.sh` does the same copy for source installs.
`./install.sh -o` is the only path that can overwrite shipped filenames you
already edited.

## Runtime Hotkeys

Configured in:

- `~/.config/phantom/config.toml`
- `[runtime_hotkeys]`

Defaults:

- `F1` -> toggle mouse routing
- `F8` -> toggle capture
- `F9` -> toggle pause
- `F10` -> toggle the experimental debug control preview
- `F2` -> shutdown daemon

Fn row warning:

- on many keyboards the function row only emits true `F1`, `F8`, and `F10` events when Fn Lock is enabled
- if `F2` works but `F1`, `F8`, or `F10` appear dead, check Fn Lock first

Overlay notes:

- `F10` shows or hides an experimental debug preview of the current profile
- on Wayland, Phantom prefers a compact passthrough HUD built from layer-shell marker surfaces
- if that path is unavailable, Phantom falls back to the older fullscreen preview window
- it is not intended for normal gameplay
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
- leaving capture now replays currently held keys to the desktop
- it no longer destroys toggle-look state
- `while_held` mouse buttons are resynced when mouse routing is turned back on
- entering capture also re-establishes currently held keyboard-driven hold controls such as `tap`, `repeat_tap`, `joystick`, and hold-mode `layer_shift`
- capture transitions release any stale desktop-relay keys before mode ownership changes, so toggling capture should not leave desktop keys logically stuck
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
- on Wayland sessions that expose wlr-layer-shell, that cursor is drawn from the desktop Xcursor theme and hides after five seconds idle
- GNOME and pure X11 still inject menu-touch but do not show that overlay
- wheel performs a short vertical swipe; unused keys type into Android
- on touchpads, Phantom synthesizes tap-to-click and double-tap-hold drag locally because those gestures are no longer provided by the desktop once Phantom owns the mouse

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

Behavior:

- joystick movement starts from the configured center and immediately drags
  outward with a long full-throw swipe
- use `drag` instead when the game expects a one-shot gesture instead of
  sustained movement

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
