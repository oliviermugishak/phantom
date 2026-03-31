# Installation Guide

## Prerequisites

| Requirement | Version | Check |
|---|---|---|
| Linux kernel | 5.1+ | `uname -r` |
| Rust toolchain | 1.75+ | `rustc --version` |
| Waydroid | any | `waydroid status` |
| Compositor | wlroots-based or X11 | `echo $XDG_SESSION_TYPE` |
| `/dev/uinput` | accessible | `ls -la /dev/uinput` |

### Kernel modules

```bash
# Verify uinput module is loaded
lsmod | grep uinput
# If not loaded:
sudo modprobe uinput

# Make it persistent across reboots
echo uinput | sudo tee /etc/modules-load.d/uinput.conf
```

### Verify Waydroid

```bash
waydroid status
# Should show: Session: RUNNING

# Waydroid must be running for Phantom to work
# If not running:
waydroid session start
```

## Build from source

```bash
# Clone
git clone <repo-url>
cd ttplayer

# Build release binary
cargo build --release

# Binaries are at:
# target/release/phantom       — daemon + CLI
# target/release/phantom-gui   — profile editor
```

## Setup

### Option A: Run as root

Simple but not recommended for daily use:

```bash
sudo ./target/release/phantom --daemon
```

### Option B: udev rules (recommended)

Allow your user to access `/dev/uinput` and `/dev/input/*` without root:

```bash
# Copy udev rules
sudo cp contrib/99-phantom.rules /etc/udev/rules.d/

# Reload
sudo udevadm control --reload-rules
sudo udevadm trigger

# Add yourself to the input group
sudo usermod -aG input $USER

# Log out and back in for group change to take effect
# Verify:
groups | grep input
```

After this, you can run without `sudo`:

```bash
./target/release/phantom --daemon
```

### Option C: Systemd service

For automatic startup:

```bash
# Copy service file
sudo cp contrib/phantom.service /etc/systemd/system/

# Edit the ExecStart path if your binary is not at /usr/local/bin/phantom
sudo nano /etc/systemd/system/phantom.service

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable --now phantom

# Check status
sudo systemctl status phantom
journalctl -u phantom -f
```

## First run

### 1. Start the daemon

```bash
# With root
sudo ./target/release/phantom --daemon

# Or without root (after udev setup)
./target/release/phantom --daemon
```

Expected output:
```
INFO  phantom 0.1.0 starting
INFO  screen resolution from /sys/class/graphics/fb0/virtual_size: 1920x1080
INFO  uinput device created: 1920x1080, 10 slots
INFO  grabbed: /dev/input/event3 (AT Translated Set 2 keyboard)
INFO  grabbed: /dev/input/event7 (Logitech USB Mouse)
INFO  captured 2 input devices
INFO  loading default profile: /home/user/.config/phantom/profiles/default.json
INFO  IPC server listening on /run/user/1000/phantom.sock
INFO  daemon ready, entering event loop
```

### 2. Load a profile

In another terminal:

```bash
./target/release/phantom load profiles/pubg.json
```

### 3. Test in Waydroid

Open Android Settings > Developer Options > Show taps (enable it).

Press keys that are bound in the profile. You should see touch indicators appear on the Waydroid screen at the positions specified in the profile.

### 4. Verify with evtest

```bash
# Find Phantom's virtual device
cat /proc/bus/input/devices | grep -A 4 "Phantom"

# Watch events
sudo evtest /dev/input/eventN  # use the event number from above
# Press a bound key — should show ABS_MT events
```

## Profile setup

### Copy example profiles

```bash
mkdir -p ~/.config/phantom/profiles
cp profiles/*.json ~/.config/phantom/profiles/

# Set a default profile (loaded on daemon start)
cp profiles/pubg.json ~/.config/phantom/profiles/default.json
```

### Create a config file

```bash
mkdir -p ~/.config/phantom
cp config.example.toml ~/.config/phantom/config.toml

# Edit if needed (screen resolution override, log level)
nano ~/.config/phantom/config.toml
```

### Use the GUI

```bash
./target/release/phantom-gui
```

1. File > Open — load a profile
2. View > Load Screenshot — load a game screenshot as background
3. Drag nodes to button positions on the screenshot
4. File > Save

## Troubleshooting

### "Permission denied on /dev/uinput"

```bash
# Check permissions
ls -la /dev/uinput
# Should be crw-rw---- root:input

# Fix: add user to input group
sudo usermod -aG input $USER
# Log out and back in
```

### "No input devices found"

```bash
# Check /dev/input permissions
ls -la /dev/input/
# Devices should be readable by input group

# Fix: apply udev rules
sudo cp contrib/99-phantom.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

### "Daemon already running"

Another Phantom instance is running. Either:
- Use it: `phantom status`
- Shutdown it: `phantom shutdown`
- Force kill: `sudo pkill phantom` (leaves orphaned uinput device — reboot to clean)

### Touches not appearing in Waydroid

```bash
# Verify uinput device was created
cat /proc/bus/input/devices | grep -A 4 "Phantom"

# If not found, daemon failed to start — check logs
# If found, verify Waydroid can see it:
# Inside Waydroid:
adb shell getevent -l
# Should show events when you press bound keys
```

### Screen resolution wrong

```bash
# Check auto-detected resolution
phantom status

# Override in config
cat > ~/.config/phantom/config.toml << EOF
[screen]
width = 2560
height = 1440
EOF
```

### Key not recognized

Key names must match the Linux input subsystem names exactly. See `docs/PROFILES.md` for the full list. Common mappings:

| Profile name | Keyboard key |
|---|---|
| `Space` | Spacebar |
| `LeftCtrl` or `Ctrl` | Left Ctrl |
| `LeftShift` or `Shift` | Left Shift |
| `Enter` | Enter/Return |
| `Esc` | Escape |
| `MouseLeft` | Left mouse button |
| `MouseRight` | Right mouse button |
| `W`, `A`, `S`, `D` | WASD keys |
| `F1`–`F12` | Function keys |
