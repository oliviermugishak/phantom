# Phantom

Phantom is a Linux keyboard-and-mouse to Android multitouch mapper for fullscreen Waydroid play.

The current recommended architecture is:

- host-side `evdev` capture for keyboard and mouse
- host-side profile engine that turns input into abstract touch commands
- Android-side touch injection through `app_process` and `InputManager.injectInputEvent()`

That path is exposed as the `android_socket` backend and is the primary backend for this project.

The older `uinput` backend still exists as a compatibility backend, but it is no longer the main design center of the project.

## What Phantom Is

Phantom is intentionally narrow:

- one known Linux machine
- one known Waydroid instance
- one fixed fullscreen Android surface
- manual, game-specific profiles

That is not a limitation by accident. It is the design that makes the mapper understandable, debuggable, and maintainable.

## What Phantom Is Not

Phantom is not trying to be:

- a general Android automation system
- a compositor plugin
- a desktop cursor remapper
- a generic emulator frontend
- a UI-recognition tool

## Current Product Shape

Runtime features:

- `evdev` keyboard and mouse capture
- runtime capture on/off
- runtime mouse routing on/off while capture stays active
- pause/resume without shutting the daemon down
- live profile load and reload
- live in-memory profile push from the GUI
- Android-side touch injection through `app_process`
- `uinput` fallback backend

Supported profile primitives:

- `tap`
- `hold_tap`
- `toggle_tap`
- `joystick`
- `mouse_camera`
- `repeat_tap`
- `macro`
- `layer_shift`

GUI features:

- screenshot-first editor
- direct control placement
- drag editing for point controls
- drag/resize editing for mouse-look regions
- runtime status chips
- live `Push Live`
- runtime capture/pause/mouse-routing buttons
- key capture from real keyboard and mouse input
- mouse-look activation mode editing

## Recommended Backend

For Waydroid in the current project state, use:

- `touch_backend = "android_socket"`

Use `uinput` only if:

- you explicitly want the legacy path
- you are debugging low-level kernel device behavior
- your environment cannot use the Android-side server path

Why:

- Android framework injection handles multi-touch state correctly inside Android
- Waydroid no longer has to discover a new virtual touchscreen device
- the project now matches how scrcpy-style input injection works

## Documentation Map

Start here, in this order:

1. [docs/INSTALL.md](docs/INSTALL.md)
   Full clean-machine setup, including Android SDK, build, config, permissions, and first startup.
2. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
   Current system design, data flow, file ownership, state model, and design decisions.
3. [docs/OPERATIONS.md](docs/OPERATIONS.md)
   Daily use: daemon commands, hotkeys, GUI workflow, capture semantics, and recommended game workflows.
4. [docs/TESTING.md](docs/TESTING.md)
   Bring-up and validation matrix for `android_socket` and `uinput`.
5. [docs/PROFILES.md](docs/PROFILES.md)
   Profile schema, node behavior, validation rules, and mapping guidelines.

Reference documents:

- [docs/IPC.md](docs/IPC.md)
- [docs/ANDROID_SOCKET_PROTOCOL.md](docs/ANDROID_SOCKET_PROTOCOL.md)
- [docs/PROTOCOL.md](docs/PROTOCOL.md)
- [docs/EDGE_CASES.md](docs/EDGE_CASES.md)
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)
- [TOTAL_SCOPE.md](TOTAL_SCOPE.md)
- [contrib/android-server/README.md](contrib/android-server/README.md)

## Quick Start

This is the short version. The full version is in [docs/INSTALL.md](docs/INSTALL.md).

1. Install Phantom into your user environment:

```bash
./install.sh
```

2. Edit `~/.config/phantom/config.toml` and set the real Android screen size.

3. Start Waydroid and make sure the container is not frozen:

```bash
waydroid session start
waydroid show-full-ui
sudo waydroid status
```

4. Start Phantom:

```bash
sudo phantom --trace --daemon
```

5. Verify status and load a profile:

```bash
phantom status
phantom audit ~/.config/phantom/profiles/pubg.json
phantom load ~/.config/phantom/profiles/pubg.json
phantom enter-capture
```

6. Open the studio:

```bash
phantom-studio
```

Use the `Runtime` tab in Phantom Studio to inspect daemon state, start the daemon, push profiles live, toggle capture, and shut the daemon down.

## Runtime Model

Phantom runtime state is easiest to reason about as five switches:

- daemon running or not
- keyboard grabbed by daemon or not
- capture enabled or not
- mouse grabbed for gameplay or not
- engine paused or not

Important distinction:

- the daemon keeps the physical keyboard grabbed while it is alive so runtime hotkeys stay reliable
- non-hotkey keyboard events are mirrored back into Linux through Phantom's virtual desktop keyboard while gameplay capture is off
- capture enabled means Phantom is forwarding gameplay input into the engine
- mouse grabbed means mouse-originated events are actually forwarded into the game

That distinction is what makes desktop adjustments, menu interaction, and future PUBG-like aim workflows possible.

## Runtime Hotkeys

Daemon hotkeys are configurable in `~/.config/phantom/config.toml`:

```toml
[runtime_hotkeys]
mouse_toggle = "F1"
capture_toggle = "F8"
pause_toggle = "F9"
shutdown = "F2"
```

Use `""` or `"none"` to disable a hotkey.

The daemon keeps the physical keyboard grabbed while it is running so these hotkeys still work even when gameplay capture is currently off. Outside gameplay capture, normal non-hotkey keyboard events are passed back to Linux through Phantom's virtual desktop keyboard.

Important keyboard note:

- on many laptops and compact keyboards, the top row only sends standard `F1`/`F8`/`F9` key events when Fn Lock is enabled
- if `F2` works but `F1` or `F8` do not, check Fn Lock first before assuming Phantom is broken

Default meaning:

- `F8`: enter or leave capture
- `F1`: toggle mouse routing while staying in capture
- `F9`: pause or resume touch injection
- `F2`: stop the daemon

## Mouse Look Modes

`mouse_camera` is Phantom's mouse-look primitive. It is touch-drag camera emulation, not desktop pointer emulation.

Supported activation modes:

- `always_on`
- `while_held`
- `toggle`

Use cases:

- `always_on`
  Good for games where capture should always steer the camera.
- `while_held`
  Good for games where a key should both enter a mode and enable look, such as aim-down-sights.
- `toggle`
  Good for games where you want to explicitly turn camera mode on and off.

## Common Commands

```bash
phantom --daemon
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
```

`phantom audit` is the fastest way to confirm whether a profile can actually hold multiple mapped touches at once, because it shows the slot layout directly from the profile model.

## Installation Notes

- `./install.sh` builds the workspace, installs `phantom`, `phantom-gui`, and the preferred studio alias `phantom-studio` into `~/.local/bin`, installs the Android server jar into `~/.local/share/phantom/android/`, and creates `~/.config/phantom/config.toml` if it does not exist yet.
- `./install.sh -u` removes the installed binaries and Android server jar, but leaves your config and profiles in `~/.config/phantom/`.
- The daemon does not auto-write `~/.config/phantom/config.toml` by itself. If the file is missing, it runs with defaults. The installer is what creates the config file for you.

## Example Profiles

Shipped starter profiles:

- `profiles/pubg.json`
- `profiles/genshin.json`
- `profiles/efootball-template.json`

Treat them as starting points, not final universal layouts.

Current `profiles/pubg.json` intent:

- `MouseLeft` -> fire
- `MouseRight` -> ADS hold and mouse-look activation
- `mouse_camera` -> `while_held` on `MouseRight`

## Current Direction

The correct direction for Phantom is:

- keep the architecture explicit
- keep the project Waydroid-focused
- keep profiles deterministic
- prefer debuggable systems over magic
- expand usability and documentation before adding scope

That is how the project stays maintainable.
