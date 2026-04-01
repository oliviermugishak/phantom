# Phantom

Keyboard and mouse to virtual multitouch mapper for fullscreen Waydroid play.

Phantom reads host input through Linux `evdev`, can enter exclusive capture on demand, maps it through a profile-driven state machine, and injects touches through a selectable backend:

- `uinput` for a host-kernel virtual touchscreen
- `android_socket` for an Android-side `app_process` touch server inside Waydroid over TCP

The backends have different startup orders and different health checks. Read [docs/TESTING.md](docs/TESTING.md) before validating one of them.

## Product Shape

Phantom is intentionally narrow:

- one Linux device
- one fullscreen Waydroid surface
- one fixed touch resolution
- manual per-game profiles

That is the supported path. The daemon now treats the touch resolution as a first-class contract instead of guessing.

## Supported Control Types

- `tap`
- `hold_tap`
- `toggle_tap`
- `joystick`
- `mouse_camera` in JSON, shown as `Mouse Look` in the UI
- `repeat_tap`
- `macro`
- `layer_shift`

This covers the normal control patterns for PUBG-like, Genshin-like, and eFootball-like games:

- WASD movement
- locked mouse look
- mouse or keyboard bound taps and holds
- repeated clicks
- toggle-style holds
- alternate layers or modes

## Runtime Features

- strict profile `screen` matching
- shared evdev observation on startup
- `F8` toggles capture on or off inside the daemon
- `F1` toggles mouse grab while capture is already active
- `F9` toggles pause or resume inside the daemon
- live profile push over IPC
- profile reload from disk
- layer switching and toggle nodes

## GUI Features

`phantom-gui` is now a native mapper, not just a raw JSON editor.

It supports:

- screenshot-first canvas editing
- bundled templates for PUBG, Genshin, and eFootball
- direct placement tools
- drag editing for points
- drag and resize handles for mouse-look regions
- mouse-wheel zoom and space-drag panning
- grid and node snapping for placement
- hover cards and right-click canvas actions
- on-the-fly key capture
- undo and redo
- copy, paste, and duplicate controls
- inline rename
- control reordering from the left panel
- pixel coordinate feedback in the properties panel
- active-layer highlighting
- macro step editing
- layer switch editing
- live daemon status
- one-click `Push Live`
- runtime buttons for capture and pause control

## Quick Start

Choose the backend first:

- `uinput`: Waydroid should see a new kernel touchscreen device
- `android_socket`: Waydroid keeps no new kernel device; Phantom talks to an Android-side server over TCP

1. Build:

```bash
cargo build --release
```

2. Enable input-device access. If you want the `uinput` backend, also enable `/dev/uinput`:

```bash
sudo modprobe uinput
echo uinput | sudo tee /etc/modules-load.d/uinput.conf

sudo cp contrib/99-phantom.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -aG input "$USER"
```

Log out and back in after changing groups.

3. Install config and profiles:

```bash
mkdir -p ~/.config/phantom/profiles
cp config.example.toml ~/.config/phantom/config.toml
cp profiles/*.json ~/.config/phantom/profiles/
cp profiles/pubg.json ~/.config/phantom/profiles/default.json
```

4. Set the real fullscreen Waydroid resolution in `~/.config/phantom/config.toml`:

```toml
log_level = "info"

[screen]
width = 1920
height = 1080
```

Default backend:

```toml
touch_backend = "uinput"
```

5. Start using the backend-specific order:

For `uinput`:

```bash
./target/release/phantom --daemon
waydroid session stop
waydroid session start
```

For `android_socket`:

```bash
waydroid session start
waydroid show-full-ui
sudo ./target/release/phantom --daemon
```

6. Check status or load another profile:

```bash
./target/release/phantom audit ~/.config/phantom/profiles/pubg.json
./target/release/phantom status
./target/release/phantom load ~/.config/phantom/profiles/pubg.json
```

7. Open the GUI:

```bash
./target/release/phantom-gui
```

8. Enter gameplay capture when you are ready to play:

```bash
./target/release/phantom enter-capture
```

Runtime workflow:

- daemon start: Phantom observes keyboard and mouse but does not seize desktop input
- `F8`: enter or leave exclusive gameplay capture
- `F1`: temporarily release or re-grab only the mouse while staying in capture mode
- `F9`: pause or resume touch injection without shutting the daemon down

Editor workflow:

- use `Bind` in the properties panel as the primary key-binding flow
- use `Templates` to start from a shipped layout instead of mapping from scratch
- use the mouse wheel to zoom and hold `Space` while dragging to pan
- right-click controls on the canvas for bind, copy, duplicate, delete, and reorder

Editor shortcuts:

- `Ctrl+N`, `Ctrl+O`, `Ctrl+S`, `Ctrl+Shift+S`
- `Ctrl+R` to push live
- `Ctrl+Z`, `Ctrl+Shift+Z`, `Ctrl+Y`
- `Ctrl+C`, `Ctrl+V`, `Ctrl+D`
- `Delete`
- `1` to `7` for Select, Tap, Hold, Toggle, Left Stick, Mouse Look, Rapid Tap

## Common Commands

```bash
phantom --daemon
phantom audit <profile.json>
phantom load <profile.json>
phantom reload
phantom status
phantom pause
phantom resume
phantom enter-capture
phantom exit-capture
phantom toggle-capture
phantom sensitivity <value>
phantom list
phantom shutdown
```

Use `phantom audit <profile.json>` before live testing when you need to confirm that the controls you care about land on distinct touch slots. This is the quickest way to answer questions like "can this profile really hold 5 mapped touches at once?" without involving Waydroid or the game yet.

## Example Profiles

- `profiles/pubg.json`
- `profiles/genshin.json`
- `profiles/efootball-template.json`

The eFootball profile is still a starter layout. Expect to tune it in the GUI for your device.

## Documentation

- [docs/INSTALL.md](docs/INSTALL.md)
- [docs/TESTING.md](docs/TESTING.md)
- [docs/ANDROID_SOCKET_PROTOCOL.md](docs/ANDROID_SOCKET_PROTOCOL.md)
- [docs/PROFILES.md](docs/PROFILES.md)
- [docs/IPC.md](docs/IPC.md)
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- [docs/PROTOCOL.md](docs/PROTOCOL.md)
- [docs/EDGE_CASES.md](docs/EDGE_CASES.md)
- [TOTAL_SCOPE.md](TOTAL_SCOPE.md)

## Android Backend

The `android_socket` backend is intended for the Waydroid case where host-side `uinput` multitouch is accepted by Phantom but still collapses downstream inside Android.

Build the Android server:

```bash
./contrib/android-server/build.sh
```

`build.sh` auto-detects the newest installed SDK platform `android.jar` from `ANDROID_JAR`, `ANDROID_SDK_ROOT`, `ANDROID_HOME`, or `~/Android/Sdk`.
It also uses `d8` to build a dex jar for `app_process`; a plain Java `.class` jar is not valid for this backend.

Then point Phantom at the built jar:

```toml
touch_backend = "android_socket"

[android]
auto_launch = true
server_jar = "/absolute/path/to/ttplayer/contrib/android-server/build/phantom-server.jar"
```

What changes when `android_socket` is enabled:

- start Waydroid first, make sure `waydroid status` shows `Container: RUNNING`, then start Phantom
- starting the daemon with `sudo` is fine; Phantom resolves config and IPC paths from the invoking user
- the daemon stages a jar into the container and launches it with `waydroid shell`
- `getevent` and `dumpsys input` no longer show a new Phantom touchscreen device
- the meaningful health signals become the Phantom daemon log, the container TCP listener, and the Android server log

Current Android backend artifacts:

- staged jar inside the container: `/data/local/tmp/phantom-server.jar`
- server log inside the container: `/data/local/tmp/phantom-server.log`
- listener port inside Waydroid: `27183` by default

Important runtime note:

- `Session: RUNNING` with `Container: FROZEN` is not enough for `android_socket`
- if the container is frozen, open Waydroid with `waydroid show-full-ui` or launch the game before starting Phantom

## Testing

```bash
cargo test
cargo clippy --workspace --all-targets
```

Ignored tests still require a real `/dev/uinput`.

## License

MIT
