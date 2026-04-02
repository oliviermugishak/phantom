# Testing

This is the practical validation guide for Phantom.

It is organized around the current primary backend:

- `android_socket`

The `uinput` backend is still testable, but it is no longer the default product path.

## 1. Build Validation

Run:

```bash
cargo fmt --all
cargo test --quiet
cargo build --release
./contrib/android-server/build.sh
```

Expected:

- Rust tests pass
- release build completes
- the Android server jar rebuilds cleanly
- the Android jar contains `classes.dex`

## 2. Profile Preflight

Before live gameplay tests, audit the profile:

```bash
phantom audit ~/.config/phantom/profiles/pubg.json
```

Look for:

- correct screen contract
- distinct logical slots for controls you expect to coexist
- correct `mouse_camera` activation mode
- intended joystick mode
- intended drag start/end positions

This is the fastest way to catch profile mistakes before involving Waydroid.

## 3. `android_socket` Bring-Up

Preconditions:

- Waydroid session started
- Waydroid container not frozen
- Phantom config points at a valid built Android server jar

Bring-up:

```bash
waydroid session start
waydroid show-full-ui
sudo waydroid status
sudo phantom --trace --daemon
```

For raw evdev/device detail only when required:

```bash
sudo env PHANTOM_TRACE_DETAIL=1 phantom --trace --daemon
```

Required Waydroid state:

- `Session: RUNNING`
- `Container: RUNNING`

### Expected Daemon Signals

Healthy startup usually includes:

- `android touch server unavailable, attempting auto-launch`
- `launching android touch server`
- `connected to android touch server`
- `touch backend ready`

### Expected Container Signals

Useful checks:

```bash
sudo waydroid shell -- sh -c 'ls -l /data/local/tmp/phantom-server.jar'
sudo waydroid shell -- sh -c 'tail -n 50 /data/local/tmp/phantom-server.log'
sudo waydroid shell -- sh -c 'ss -ltnp | grep 27183 || true'
```

Healthy signals:

- staged jar exists
- server log shows startup
- server log shows a client connection
- port `27183` is listening

## 4. Hotkey Validation

After the daemon is up, check:

- `F1`
- `F8`
- `F9`
- `F10`
- `F2`

If `F2` works but `F1`, `F8`, or `F10` do not:

- check Fn Lock first

That is a keyboard behavior issue on many compact keyboards, not usually a Phantom backend failure.

For `F10`, also remember:

- the current overlay is an experimental host-side debug window
- if it does not appear, inspect `~/.config/phantom/overlay.log`
- if the log mentions missing `WAYLAND_DISPLAY` or `DISPLAY`, the overlay child was launched without a usable desktop session environment

## 5. GUI Profile Discovery Validation

Open:

```bash
phantom-gui
```

Confirm:

- profiles from `~/.config/phantom/profiles/` appear in the GUI
- newly shipped profiles appear after rerunning `./install.sh`
- save/open defaults to the user profile library

## 6. Gameplay Validation Matrix

Run these in order:

1. single tap
2. hold one control
3. hold one control and tap another
4. hold two controls at once
5. double-tap style action
6. joystick plus button
7. mouse-look plus button
8. floating joystick movement
9. one-shot drag gesture
10. layer shift or macro, if used

This order isolates:

- transport failures
- profile failures
- game-specific layout failures

For complex shooter profiles, validate each named layer independently after the base combat layer is stable. See [GAME_PATTERNS.md](GAME_PATTERNS.md).

## 7. Profile-Specific Smoke Tests

### PUBG Mobile

Test:

- `WASD` movement
- `LeftShift` sprint-lock drag if the profile uses it
- `MouseRight` mouse-look activation
- `MouseLeft` firing while looking
- jump, crouch, prone, interact

### Temple Run / Subway Surfers

Test:

- `A` swipe left
- `D` swipe right
- `W` swipe up
- `S` swipe down

### Asphalt 8 / Asphalt 9

Test:

- left/right steering
- brake
- nitro
- drift

These are keyboard starter layouts, not a dedicated analog steering-wheel implementation.

## 8. Mouse Look Validation

### `always_on`

Expected:

- capture + mouse routing immediately drives the look region

### `while_held`

Expected:

- mouse movement does nothing until the activation key is held
- releasing the activation key emits a clean `TouchUp`

### `toggle`

Expected:

- first press enables look
- second press disables look
- disabling emits a clean `TouchUp`

## 9. Failure Mapping

### Daemon Never Connects To The Android Server

Check:

- Waydroid state
- configured Android server jar path
- `/data/local/tmp/phantom-server.log`

### GUI Does Not Show A New Profile

Check:

- the file exists in `~/.config/phantom/profiles/`
- rerun `./install.sh` to seed newly shipped profiles
- restart `phantom-gui`

### Touch Placement Is Wrong

Check:

- daemon screen contract
- profile screen contract
- actual Waydroid surface size

### Temple Run Tilt Does Not Work

This is expected.

Reason:

- Phantom injects touch
- tilt is accelerometer input

## 10. Success Signals

A healthy run looks like this:

- daemon sees the input
- the engine emits the expected touch commands
- the Android backend connects cleanly
- simultaneous touches remain active when expected
- the game behaves as if the touches are truly concurrent
