# Phantom

Keyboard and mouse to virtual multitouch mapper for fullscreen Waydroid play.

Phantom reads host input through Linux `evdev`, can enter exclusive capture on demand, maps it through a profile-driven state machine, and injects MT Protocol B touch events through `uinput` so Waydroid sees a direct-touch device.

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

1. Build:

```bash
cargo build --release
```

2. Enable `uinput` and input-device access:

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

5. Start Phantom, then restart Waydroid if it is already running:

```bash
./target/release/phantom --daemon
waydroid session stop
waydroid session start
```

6. Check status or load another profile:

```bash
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

## Example Profiles

- `profiles/pubg.json`
- `profiles/genshin.json`
- `profiles/efootball-template.json`

The eFootball profile is still a starter layout. Expect to tune it in the GUI for your device.

## Documentation

- [docs/INSTALL.md](docs/INSTALL.md)
- [docs/PROFILES.md](docs/PROFILES.md)
- [docs/IPC.md](docs/IPC.md)
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- [docs/PROTOCOL.md](docs/PROTOCOL.md)
- [docs/EDGE_CASES.md](docs/EDGE_CASES.md)
- [TOTAL_SCOPE.md](TOTAL_SCOPE.md)

## Testing

```bash
cargo test
cargo clippy --workspace --all-targets
```

Ignored tests still require a real `/dev/uinput`.

## License

MIT
