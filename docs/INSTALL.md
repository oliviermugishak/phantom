# Installation And Integration

This guide covers the full setup for the current Phantom product shape: fullscreen Waydroid on one known touch resolution.

## Requirements

| Requirement | Notes |
|---|---|
| Linux kernel with `uinput` | required only for the `uinput` backend |
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
- `contrib/android-server/build/phantom-server.jar` if you build the Android backend server

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

If you plan to use the Android framework backend instead of `uinput`, you still need access to `/dev/input/event*`, but you do not need `/dev/uinput`.

## Config And Profiles

```bash
mkdir -p ~/.config/phantom/profiles
cp config.example.toml ~/.config/phantom/config.toml
cp profiles/*.json ~/.config/phantom/profiles/
cp profiles/pubg.json ~/.config/phantom/profiles/default.json
```

Set the real fullscreen Waydroid resolution in `~/.config/phantom/config.toml`:

```toml
log_level = "info"

[screen]
width = 1920
height = 1080
```

Optional backend selection:

```toml
touch_backend = "uinput"
```

This is no longer optional in practice. Phantom uses the configured screen contract instead of falling back to framebuffer guessing.

Every real profile should also carry a matching `screen` block. If the profile and daemon disagree, the load is rejected.

Before live testing, audit the profile once:

```bash
./target/release/phantom audit ~/.config/phantom/profiles/pubg.json
```

This prints the occupied touch slots, the node using each slot, the layer, and the triggering bindings. It is the fastest way to confirm that two or five controls you expect to hold simultaneously really map to distinct touch slots.

## Recommended Startup Order

The order depends on backend choice.

### `uinput`

1. Start Phantom.
2. Start or restart the Waydroid session.
3. Load the target profile if needed.
4. Enter capture when you are ready to play.
5. Verify touch placement in Android.

Commands:

```bash
./target/release/phantom --daemon
./target/release/phantom audit ~/.config/phantom/profiles/pubg.json
waydroid session stop
waydroid session start
./target/release/phantom load ~/.config/phantom/profiles/pubg.json
./target/release/phantom enter-capture
```

If Waydroid was already running when Phantom started and the touch device does not appear, restart the Waydroid session.

### `android_socket`

1. Start or confirm the Waydroid session is already running.
2. Start Phantom.
3. Let Phantom stage and launch the Android server.
4. Load the target profile if needed.
5. Enter capture.

Commands:

```bash
waydroid session start
waydroid show-full-ui
sudo ./target/release/phantom --trace --daemon
./target/release/phantom audit ~/.config/phantom/profiles/pubg.json
./target/release/phantom load ~/.config/phantom/profiles/pubg.json
./target/release/phantom enter-capture
```

If Phantom is started with `sudo`, it now resolves config and IPC paths from the invoking user, so the normal user can still run CLI commands and use the GUI against the daemon.
`Session: RUNNING` is not enough for this backend: if `waydroid status` also shows `Container: FROZEN`, open the UI or launch the game first so the container is actually responsive.

## Android Framework Backend

For Waydroid setups where host-side `uinput` touches are accepted by Phantom but still collapse downstream, Phantom can target an Android-side `app_process` server instead.

Build the server:

```bash
./contrib/android-server/build.sh
```

This produces a dex jar for `app_process`. If the jar does not contain `classes.dex`, Phantom will now reject auto-launch and ask you to rebuild it.

Then configure:

```toml
touch_backend = "android_socket"

[android]
auto_launch = true
server_jar = "/absolute/path/to/ttplayer/contrib/android-server/build/phantom-server.jar"
#host = "192.168.240.112"
#port = 27183
```

Default Android backend runtime values:

- host: Waydroid container IP from `waydroid status`
- port: `27183`
- staged server jar inside the container: `/data/local/tmp/phantom-server.jar`
- server log inside the container: `/data/local/tmp/phantom-server.log`

Expected daemon startup signals:

- `touch backend ready` with `backend=android_socket`
- `android touch server unavailable, attempting auto-launch` on a cold start
- `launching android touch server`
- `connected to android touch server`

Expected server log signals:

- `phantom-server starting host=0.0.0.0 port=27183`
- `phantom-server client connected`

## First Verification

Backend matters here. Do not mix the checks.

### `uinput`

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
waydroid shell getevent -lp | grep -A10 "Phantom"
```

### 4. Confirm Android receives touches

Enable Android `Show taps` in Developer Options, then press mapped keys.

Expected behavior:

- `tap`, `hold_tap`, and `toggle_tap` land on fixed buttons
- `joystick` holds and moves from a fixed left-stick center
- `mouse_camera` drives a bounded mouse-look swipe region
- `repeat_tap` repeatedly presses while held

### `android_socket`

1. Confirm the daemon is alive:

```bash
./target/release/phantom status
```

2. Confirm the Android server assets exist inside the container:

```bash
sudo waydroid shell -- sh -c 'ls -l /data/local/tmp/phantom-server.jar'
sudo waydroid shell -- sh -c 'ss -ltnp | grep 27183 || true'
sudo waydroid shell -- sh -c 'tail -n 50 /data/local/tmp/phantom-server.log'
sudo waydroid status
```

The expected status for a live backend is:

- `Session: RUNNING`
- `Container: RUNNING`

If the container is frozen, the TCP server may listen but still fail Phantom's readiness ping until the UI is opened.

3. Confirm touch reaches the game by using a mapped key.

For `android_socket`, `getevent` and `dumpsys input` are not the primary verification path, because there is no new kernel input device to inspect.

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
waydroid shell getevent -lp | grep -A10 "Phantom"
```

If the host sees the device but Waydroid does not:

- start Phantom first
- restart the Waydroid session
- keep Waydroid on the intended fullscreen surface

### `android_socket` daemon cannot connect

Check:

```bash
sudo waydroid status
sudo waydroid shell -- sh -c 'ss -ltnp | grep 27183 || true'
sudo waydroid shell -- sh -c 'tail -n 100 /data/local/tmp/phantom-server.log'
```

Common causes:

- Waydroid session was not running before daemon start
- Phantom could not stage the server jar into `/data/local/tmp`
- `waydroid shell` failed
- ART cold start took too long and the daemon timed out
- the server crashed during `app_process` startup

If the server dies mid-session, restart the Phantom daemon. The current backend does not yet reconnect to a freshly restarted server automatically.

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
