# Installation And Integration

This guide covers the full setup for the current Phantom product shape: fullscreen Waydroid on one known touch resolution.

## Requirements

| Requirement | Notes |
|---|---|
| Linux kernel with `uinput` | Phantom uses a virtual direct-touch device |
| Access to `/dev/input/event*` and `/dev/uinput` | root or `input` group |
| Waydroid | host and container share the same kernel input stack |
| Rust toolchain | needed to build from source |
| One known fullscreen resolution | Phantom now expects this explicitly |

## Build

```bash
git clone <repo-url>
cd ttplayer
cargo build --release
```

Artifacts:

- `target/release/phantom`
- `target/release/phantom-gui`

## Host Setup

### 1. Enable `uinput`

```bash
sudo modprobe uinput
echo uinput | sudo tee /etc/modules-load.d/uinput.conf
```

Verify:

```bash
ls -l /dev/uinput
```

### 2. Grant access to input devices

```bash
sudo cp contrib/99-phantom.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -aG input "$USER"
```

Log out and back in, then verify:

```bash
groups | grep input
ls -l /dev/uinput
ls -l /dev/input/event*
```

You can also run the daemon as root:

```bash
sudo ./target/release/phantom --daemon
```

## Config And Profiles

```bash
mkdir -p ~/.config/phantom/profiles
cp profiles/*.json ~/.config/phantom/profiles/
cp profiles/pubg.json ~/.config/phantom/profiles/default.json
```

On first daemon start, Phantom auto-creates `~/.config/phantom/config.toml` for the invoking user if it is missing.

Set the real fullscreen Waydroid resolution in `~/.config/phantom/config.toml`:

```toml
log_level = "info"

[screen]
width = 1920
height = 1080
```

This is no longer optional in practice. Phantom uses the configured screen contract instead of falling back to framebuffer guessing.

Every real profile should also carry a matching `screen` block. If the profile and daemon disagree, the load is rejected.

## Recommended Startup Order

1. Start Phantom.
2. Edit the generated config if your fullscreen surface is not `1920x1080`, then restart Phantom.
3. Start or restart the Waydroid session.
4. Load the target profile if needed.
5. Enter capture when you are ready to play.
6. Verify touch placement in Android.

Commands:

```bash
sudo ./target/release/phantom --daemon
waydroid session stop
waydroid session start
./target/release/phantom load ~/.config/phantom/profiles/pubg.json
./target/release/phantom enter-capture
```

If Waydroid was already running when Phantom started and the touch device does not appear, restart the Waydroid session.

When the daemon is started with `sudo`, Phantom now resolves config, profiles, and the IPC socket against the invoking user's state instead of `/root`.
If `~/.config/phantom/config.toml` does not exist yet, Phantom creates it automatically from the shipped example and keeps it owned by the invoking user.

## First Verification

### 1. Confirm the host created the touchscreen

```bash
grep -A5 "Phantom Virtual Touch" /proc/bus/input/devices
```

### 2. Confirm the daemon is alive

```bash
./target/release/phantom status
```

You should now see:

- loaded profile name
- paused state
- capture state
- locked screen size

### 3. Confirm Waydroid can see the device

```bash
sudo waydroid shell getevent -lp | grep -A10 "Phantom"
```

If Android still does not treat Phantom as a proper touchscreen, install the shipped IDC:

```bash
phantom waydroid-print-idc
sudo phantom waydroid-install-idc
sudo phantom waydroid-diagnose
waydroid session stop
waydroid session start
```

### 4. Confirm Android receives touches

Enable Android `Show taps` in Developer Options, then press mapped keys.

Expected behavior:

- `tap` presses and releases automatically after a short pulse
- `hold_tap` stays down while the key is held
- `toggle_tap` stays active until toggled off
- `joystick` holds and moves from a fixed left-stick center
- `mouse_camera` drives a bounded mouse-look swipe region
- `repeat_tap` repeatedly presses while held

## Using The GUI

```bash
./target/release/phantom-gui
```

Recommended workflow:

1. Open or create a profile.
2. Confirm the locked screen resolution in the left panel.
3. Start from `Templates` if the game is close to one of the shipped layouts.
4. Load a screenshot from the target game if you want visual placement.
5. Use the toolbar placement tools or the `1` to `7` shortcuts:
   - `Tap`
   - `Hold`
   - `Toggle`
   - `Left Stick`
   - `Mouse Look`
   - `Rapid Tap`
6. Click on the canvas to place controls.
7. Select a control and press `Bind` in the properties panel, then press the real key or mouse button.
8. Use the mouse wheel to zoom, hold `Space` while dragging to pan, and right-click controls for quick actions.
9. Use `Push Live` to send the current in-memory profile straight to the daemon.
10. Use `Enter Capture` or `Exit Capture` from the toolbar as needed.

The GUI is now runtime-aware:

- it can query daemon status
- it can push unsaved edits live
- it can enter or exit capture mode
- it can pause or resume play through the daemon buttons
- it highlights currently active layers reported by the daemon

Editor shortcuts:

- `Ctrl+Z`, `Ctrl+Shift+Z`, `Ctrl+Y`
- `Ctrl+C`, `Ctrl+V`, `Ctrl+D`
- `Delete`

## Runtime Controls

Daemon hotkeys:

- `F1` toggles mouse grab while capture is already active
- `F8` toggles capture
- `F9` toggles pause

Recommended meaning:

- daemon startup: Phantom observes evdev input without exclusive grab
- `F8`: primary game-mode switch, enters or exits exclusive capture
- `F1`: temporary mouse release for adjustments while keyboard capture stays active
- `F9`: emergency pause for touch injection without leaving the running session

CLI controls:

```bash
phantom pause
phantom resume
phantom enter-capture
phantom exit-capture
phantom toggle-capture
phantom waydroid-print-idc
sudo phantom waydroid-install-idc
sudo phantom waydroid-diagnose
```

## Systemd

The shipped service file is a basic example:

```bash
sudo cp contrib/phantom.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now phantom
```

If you use systemd:

- install the binary somewhere stable, such as `/usr/local/bin/phantom`
- update `ExecStart=` if needed
- keep the configured `screen` resolution in sync with your real Waydroid target surface
- restart the Waydroid session after enabling the service if the touch device is not visible

## Troubleshooting

### Permission denied on `/dev/uinput`

Check:

```bash
ls -l /dev/uinput
```

Fix:

- add your user to `input`
- reapply `contrib/99-phantom.rules`
- or run the daemon as root

### No input devices found

Check:

```bash
ls -l /dev/input/event*
```

Common causes:

- missing group membership
- device access denied
- unusual hardware exposure path

### Waydroid does not react

Check both sides:

```bash
grep -A5 "Phantom Virtual Touch" /proc/bus/input/devices
sudo waydroid shell getevent -lp | grep -A10 "Phantom"
```

If the host sees the device but Waydroid does not:

- start Phantom first
- restart the Waydroid session
- keep Waydroid on the intended fullscreen surface

### Touches land in the wrong place

Your configured daemon `screen` and the profile `screen` must match the real Android surface.

Fix:

- set `[screen]` in `~/.config/phantom/config.toml`
- keep the same `screen` in the profile
- restart the daemon
- push or reload the profile again

### Desktop input disappears

That is expected while capture is enabled.

Use one of:

- press `F8`
- `phantom exit-capture`
- `phantom pause`
- a second TTY or SSH shell

### New keyboard or mouse plugged in after startup

Current limitation.

Restart the daemon after plugging in a new input device.
