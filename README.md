# Phantom

Phantom is a Linux keyboard-and-mouse to Android multitouch mapper for Waydroid.

It captures local `evdev` input on the host, maps that input through deterministic JSON profiles, and injects Android `MotionEvent`s inside the Waydroid container through an Android-side `app_process` server.

Code hosting:

- Source: `https://github.com/oliviermugishak/phantom.git`

Credits:

- Olivier Mugisha K
- GitHub: `https://github.com/oliviermugishak`

## Current Architecture

The current recommended runtime path is:

- host-side `evdev` capture
- host-side profile engine
- Android-side touch injection through `InputManager.injectInputEvent()`

That is exposed as:

- `touch_backend = "android_socket"`

The older `uinput` path still exists, but it is now the fallback backend, not the main design center.

## What Phantom Supports

Runtime features:

- keyboard and mouse capture from Linux `evdev`
- runtime capture on/off
- runtime mouse routing on/off
- live profile load and reload
- GUI-driven `Push Live`
- Android-side touch injection through `app_process`
- `uinput` fallback backend

Profile primitives:

- `tap`
- `toggle_tap`
- `joystick`
- `drag`
- `aim`
- `repeat_tap`
- `wheel`
- `macro`
- `layer_shift`

Compatibility note:

- legacy `hold_tap` profile entries still load, but Phantom now treats them as standard `tap` nodes

Important recent additions:

- `joystick` now supports both `fixed` and `floating` modes
- `drag` now supports swipe-style games such as Temple Run and Subway Surfers
- capture-on mouse navigation now defaults to owned menu-touch while gameplay aim is inactive
- macros now support explicit run modes so a sequence can either cancel on key release or continue as a one-shot
- GUI profile discovery now reads the real user profile library from `~/.config/phantom/profiles/`

## Shipped Profile Library

The repository ships starter profiles in [`profiles/`](profiles/):

- `pubg.json`
- `pubg-small.json`
- `genshin.json`
- `efootball-template.json`
- `temple-run.json`
- `subway-surfers.json`
- `asphalt8.json`
- `asphalt9.json`

These are starter layouts, not universal final configs.

## Profile Library And GUI Discovery

There are two profile locations to understand:

- repository profiles: `./profiles/*.json`
- user profile library: `~/.config/phantom/profiles/*.json`

The GUI loads profiles from the user library, not directly from the repository.

`./install.sh` handles the sync:

- it copies every shipped profile into `~/.config/phantom/profiles/` if that file does not already exist
- it does not overwrite profiles you already edited
- `./install.sh -o` can prompt to overwrite the current config and/or the currently shipped profile filenames
- rerunning `./install.sh` is the supported way to seed newly added shipped profiles into an existing setup
- rerunning `./install.sh` also rewrites `android.server_jar` to the installed jar if the existing config still points at a source-tree `contrib/android-server/build/phantom-server.jar`

That means:

- if a profile exists in the repo but does not appear in the GUI, rerun `./install.sh`
- restarting the GUI reloads the current contents of `~/.config/phantom/profiles/`

## Quick Start

1. Install Phantom into your user environment:

```bash
./install.sh
```

2. Edit `~/.config/phantom/config.toml` and set the real Waydroid screen size.
   If `android.server_jar` is stale or omitted, Phantom now falls back to the
   installed jar in `~/.local/share/phantom/android/` or a built jar in the
   current source tree.

3. Start Waydroid and make sure the container is not frozen:

```bash
waydroid session start
waydroid show-full-ui
sudo waydroid status
```

Before starting Phantom, confirm:

- `Session: RUNNING`
- `Container: RUNNING`

4. Start Phantom:

```bash
sudo phantom --trace --daemon
```

For deeper raw-device tracing only when needed:

```bash
sudo env PHANTOM_TRACE_DETAIL=1 phantom --trace --daemon
```

5. Verify status and load a profile:

```bash
phantom status
phantom audit ~/.config/phantom/profiles/pubg.json
phantom load ~/.config/phantom/profiles/pubg.json
phantom enter-capture
```

6. Open the GUI:

```bash
phantom-gui
```

## Runtime Hotkeys

Default daemon hotkeys:

- `F1` -> toggle mouse routing
- `F8` -> toggle capture
- `F9` -> toggle pause
- `F10` -> toggle the experimental debug control preview
- `F2` -> shutdown daemon

These are configured in:

- `~/.config/phantom/config.toml`
- `[runtime_hotkeys]`

Important keyboard note:

- on many laptops and compact keyboards, the top row only sends standard `F1`/`F8`/`F9`/`F10` events when Fn Lock is enabled
- if `F2` works but `F1`, `F8`, or `F10` do not, check Fn Lock first
- when capture is toggled, Phantom now flushes any stale desktop-relay key state before switching modes so the desktop does not keep phantom-held keys stuck down

## Overlay Preview

Press `F10` while the daemon is running to show or hide the experimental debug control preview.

What it shows:

- button controls as soft circles with their bound key labels
- joysticks as fixed centers or floating zones
- drag gestures as subtle swipe arrows
- aim anchors as lightweight debug markers

Important:

- on Wayland, Phantom now prefers a compositor-native passthrough HUD made of compact marker surfaces positioned over the current game/client frame
- if that Wayland HUD path is unavailable, Phantom falls back to the older fullscreen preview window
- it is still a debug surface, not an Android in-surface overlay
- it is intended for brief previewing and debugging, not for normal gameplay
- overlay launcher output is written to `~/.config/phantom/overlay.log`

Current product direction:

- the current preview surface is still experimental and may be replaced later
- the preferred long-term direction is an Android-side in-surface overlay and is tracked in [docs/ROADMAD.md](docs/ROADMAD.md)

## Tracing And Logging

Normal guidance:

- use `sudo phantom --daemon` for day-to-day runtime use
- use `sudo phantom --trace --daemon` when you need lifecycle, translated input, engine, and injection logs

Detail mode:

- `PHANTOM_TRACE_DETAIL=1` enables the raw/noisy per-device trace path
- that includes raw evdev events, touchpad re-anchor suppression, and dropped-event detail
- only use it when debugging low-level input behavior

## Menu Touch And Aim

When capture is active and gameplay aim is inactive, Phantom now treats the host mouse as owned menu-touch navigation.

What that means:

- left click becomes touch down / touch up
- mouse motion while held becomes touch drag
- this is the intended way to navigate menus in games that reject raw mouse input
- Phantom shows a separate owned menu-touch cursor while this mode is active
- on Wayland compositors such as Hyprland, that cursor is drawn through a dedicated layer-shell overlay with input passthrough
- `F1` switches between gameplay aim and owned menu-touch
- when Phantom enters menu-touch, it seeds the owned cursor from the current host cursor position when possible
- after that seed, Phantom owns the mouse and drives menu-touch from its internal cursor instead of relying on host click delivery
- the seed path prefers Hyprland compositor geometry, then X11/XWayland helper mapping, then finally Phantom's internal cursor state
- when Phantom owns a touchpad in menu-touch, it also provides its own tap-to-click and double-tap-hold drag behavior
- `phantom status` shows:

Gameplay note:

- for high-paced shooter aim, a real mouse is still the recommended hardware path
- touchpad aim remains best-effort and should be treated as a fallback, not the premium experience
  - `menu touch backend`
  - `mouse mode`
- because Phantom now owns the mouse during capture, menu-touch no longer depends on a first host click being consumed for window activation

## Aim

`aim` is Phantom's camera/look primitive. Older profiles may still use the legacy `mouse_camera` type, which Phantom accepts and normalizes on load.

Important:

- Phantom owns the physical mouse while capture is active and switches that owned mouse between `aim` and `menu_touch` modes
- actual camera movement only happens if the loaded profile contains an `aim` node
- when aim is inactive while capture stays on, Phantom stays in owned menu-touch UI navigation
- touchpads are supported, but a real mouse will usually feel smoother for camera movement
- real mouse deltas are fed to aim one evdev report at a time, with X/Y from the
  same report handled together instead of as separate jumps
- the real-mouse aim path now uses an immediate mouse-first response curve:
  tiny motions are damped for precision while larger sweeps still turn fast,
  without letting a fast vertical pull inflate tiny sideways noise
- real mouse aim is now allowed a wider hidden-touch envelope than absolute
  touchpad aim, which reduces re-centering during fast camera turns
- large camera sweeps are no longer clipped by the old fixed aim
  re-segmentation loop, and `while_held` re-engage starts from a fresh center
  instead of resuming from a stale edge
- `F1` now preserves toggle-look state and resyncs `while_held` mouse buttons when routing is restored
- entering capture also resyncs currently held keyboard controls for hold-style nodes such as `tap`, `repeat_tap`, `joystick`, and hold-mode `layer_shift`
- `phantom status` shows whether menu touch is active, which backend seeded the owned cursor, and which runtime mouse mode is active

Supported activation modes:

- `always_on`
- `while_held`
- `toggle`

Typical use:

- `always_on` for always-look-on games
- `while_held` for ADS-style aim workflows
- `toggle` for explicit look-mode switching

## Swipe And Floating-Stick Games

Phantom now supports:

- floating movement zones through `joystick` with `mode = "floating"`
- visible fixed sticks through immediate two-frame `joystick` drag engage in `mode = "fixed"`
- one-shot swipes and drags through `drag`

That makes it viable for:

- Temple Run
- Subway Surfers
- football-style floating movement zones
- sprint-lock drags in games like PUBG Mobile

## What Phantom Does Not Support

Phantom currently injects touch, not sensors.

That means:

- accelerometer tilt is not supported
- Temple Run-style tilt-to-collect-coins is not currently a Phantom feature

If a game requires sensor input and has no touch alternative, that is a separate subsystem, not a profile tweak.

## Documentation Map

Read these in this order:

1. [docs/INSTALL.md](docs/INSTALL.md)
2. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
3. [docs/OPERATIONS.md](docs/OPERATIONS.md)
4. [docs/TESTING.md](docs/TESTING.md)
5. [docs/PROFILES.md](docs/PROFILES.md)
6. [docs/GAME_PATTERNS.md](docs/GAME_PATTERNS.md)
7. [docs/TROUBLESHOOT.md](docs/TROUBLESHOOT.md)
8. [docs/EDGE_CASES.md](docs/EDGE_CASES.md)
9. [docs/ROADMAD.md](docs/ROADMAD.md)

Reference docs:

- [docs/IPC.md](docs/IPC.md)
- [docs/ANDROID_SOCKET_PROTOCOL.md](docs/ANDROID_SOCKET_PROTOCOL.md)
- [docs/PROTOCOL.md](docs/PROTOCOL.md)
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)
- [contrib/android-server/README.md](contrib/android-server/README.md)

Contribution docs:

- [CONTRIBUTING.md](CONTRIBUTING.md)
- [AGENTS.md](AGENTS.md)

Package docs:

- [phantom/README.md](phantom/README.md)
- [phantom-gui/README.md](phantom-gui/README.md)
- [contrib/README.md](contrib/README.md)

## Common Commands

```bash
phantom --daemon
phantom --version
phantom version
phantom audit <profile.json>
phantom status
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
phantom-gui
phantom-gui --version
```

## Install Notes

- `./install.sh` builds the workspace, installs `phantom` and `phantom-gui` into `~/.local/bin`, installs a sudo-visible `phantom` launcher into `/usr/local/bin` when possible, installs the Android server jar into `~/.local/share/phantom/android/`, creates `~/.config/phantom/config.toml` if missing, refreshes `android.server_jar` in the existing config when it still points at a source-tree jar, and seeds missing shipped profiles into `~/.config/phantom/profiles/`.
- `./install.sh -o` interactively asks whether to overwrite `~/.config/phantom/config.toml` and whether to overwrite the currently shipped profile filenames in `~/.config/phantom/profiles/`.
- `./install.sh -u` removes the installed binaries, the sudo-visible `phantom` launcher, and the Android server jar, but leaves your config and user profiles untouched.
- rerunning `./install.sh` is safe for profile seeding because it only copies missing shipped profiles, and it only updates `android.server_jar` automatically when that field still points at a source-tree jar

## Current Direction

The project direction remains:

- explicit screen contracts
- explicit runtime state
- deterministic profiles
- Android-first injection
- strong documentation and maintainability over opaque magic
