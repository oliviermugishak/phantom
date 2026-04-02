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
- `hold_tap`
- `toggle_tap`
- `joystick`
- `drag`
- `mouse_camera`
- `repeat_tap`
- `macro`
- `layer_shift`

Important recent additions:

- `joystick` now supports both `fixed` and `floating` modes
- `drag` now supports swipe-style games such as Temple Run and Subway Surfers
- GUI profile discovery now reads the real user profile library from `~/.config/phantom/profiles/`

## Shipped Profile Library

The repository ships starter profiles in [`profiles/`](profiles/):

- `pubg.json`
- `pubg-mobile-layout1.json`
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
- rerunning `./install.sh` is the supported way to seed newly added shipped profiles into an existing setup

That means:

- if a profile exists in the repo but does not appear in the GUI, rerun `./install.sh`
- restarting the GUI reloads the current contents of `~/.config/phantom/profiles/`

## Quick Start

1. Install Phantom into your user environment:

```bash
./install.sh
```

2. Edit `~/.config/phantom/config.toml` and set the real Waydroid screen size.

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

5. Verify status and load a profile:

```bash
phantom status
phantom audit ~/.config/phantom/profiles/pubg-mobile-layout1.json
phantom load ~/.config/phantom/profiles/pubg-mobile-layout1.json
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
- `F10` -> toggle the transparent control preview overlay
- `F2` -> shutdown daemon

These are configured in:

- `~/.config/phantom/config.toml`
- `[runtime_hotkeys]`

Important keyboard note:

- on many laptops and compact keyboards, the top row only sends standard `F1`/`F8`/`F9`/`F10` events when Fn Lock is enabled
- if `F2` works but `F1`, `F8`, or `F10` do not, check Fn Lock first

## Overlay Preview

Press `F10` while the daemon is running to show or hide a transparent click-through preview overlay.

What it shows:

- button controls as soft circles with their bound key labels
- joysticks as fixed centers or floating zones
- drag gestures as subtle swipe arrows
- mouse-look as a faint region outline

What it does not do:

- it does not block clicks or touches behind it
- it does not inject any input by itself
- it is a static preview of the currently loaded profile

## Mouse Look

`mouse_camera` is Phantom's camera/look primitive. It is touch-drag camera emulation, not desktop pointer emulation.

Important:

- runtime mouse grab only routes host mouse input into Phantom
- actual camera movement only happens if the loaded profile contains a `mouse_camera` node
- touchpads are supported, but a real mouse will usually feel smoother for camera movement

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
6. [docs/TROUBLESHOOT.md](docs/TROUBLESHOOT.md)
7. [docs/EDGE_CASES.md](docs/EDGE_CASES.md)

Reference docs:

- [docs/IPC.md](docs/IPC.md)
- [docs/ANDROID_SOCKET_PROTOCOL.md](docs/ANDROID_SOCKET_PROTOCOL.md)
- [docs/PROTOCOL.md](docs/PROTOCOL.md)
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)
- [contrib/android-server/README.md](contrib/android-server/README.md)
- [TOTAL_SCOPE.md](TOTAL_SCOPE.md)

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

- `./install.sh` builds the workspace, installs `phantom` and `phantom-gui` into `~/.local/bin`, installs a sudo-visible `phantom` launcher into `/usr/local/bin` when possible, installs the Android server jar into `~/.local/share/phantom/android/`, creates `~/.config/phantom/config.toml` if missing, and seeds missing shipped profiles into `~/.config/phantom/profiles/`.
- `./install.sh -u` removes the installed binaries, the sudo-visible `phantom` launcher, and the Android server jar, but leaves your config and user profiles untouched.
- rerunning `./install.sh` is safe for profile seeding because it only copies missing shipped profiles

## Current Direction

The project direction remains:

- explicit screen contracts
- explicit runtime state
- deterministic profiles
- Android-first injection
- strong documentation and maintainability over opaque magic
