# Testing

This is the practical validation guide for Phantom.

It is organized around the current primary backend:

- `android_socket`

The `uinput` backend is covered at the end as a compatibility appendix.

## 1. Test Layers

Treat testing in three layers:

1. build correctness
2. backend bring-up
3. gameplay behavior

Do not skip straight to gameplay if the backend is not healthy.

## 2. Build Validation

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
- Android server jar is rebuilt

The Android jar must contain `classes.dex`.

## 3. Profile Preflight

Before live runtime tests, audit the target profile:

```bash
./target/release/phantom audit ~/.config/phantom/profiles/pubg.json
```

Look for:

- distinct slots for controls you expect to hold simultaneously
- correct layer ownership
- `mouse_camera` mode and activation key
- no accidental slot reuse

This is the fastest way to catch profile mistakes before involving Waydroid.

## 4. `android_socket` Bring-Up

### Preconditions

Confirm:

- Waydroid session started
- Waydroid container unfrozen
- Phantom config points at the built Android server jar

Start sequence:

```bash
waydroid session start
waydroid show-full-ui
sudo waydroid status
sudo ./target/release/phantom --trace --daemon
```

Required Waydroid state:

- `Session: RUNNING`
- `Container: RUNNING`

If the container is `FROZEN`, do not trust a socket or listener alone. Phantom's readiness check depends on a real response from the Android side.

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
- server log shows client connected
- port `27183` is listening

## 5. Runtime Smoke Test

After the daemon is alive:

```bash
./target/release/phantom status
./target/release/phantom load ~/.config/phantom/profiles/pubg.json
./target/release/phantom enter-capture
```

Verify in `phantom status`:

- profile name
- capture state
- mouse routed state
- keyboard routed state
- screen contract

## 6. Gameplay Validation Matrix

Run these in order:

1. single tap
2. hold one control
3. hold one control and tap another
4. hold two controls at once
5. hold three controls at once, if the profile supports it
6. double-tap style action
7. joystick plus button
8. mouse-look plus button
9. mouse-look mode transition
10. layer shift or macro, if used

Why this order:

- it validates the backend incrementally
- it isolates profile mistakes from transport mistakes

## 7. Mouse Look Validation

Test according to the configured mode.

### `always_on`

Expected:

- once capture is active and mouse routing is enabled, mouse movement produces look drag immediately

### `while_held`

Expected:

- mouse movement does nothing while the activation key is not held
- pressing the activation key enables look
- releasing the activation key emits a clean `TouchUp`

### `toggle`

Expected:

- first press of the activation key enables look
- second press disables look
- disabling emits a clean `TouchUp`

## 8. Success Signals

A healthy run looks like this:

- daemon sees the physical input
- engine emits the expected `TouchCommand`s
- multiple active slots stay alive concurrently
- the game reacts as if the touches are truly simultaneous

For concurrent touch tests, the trace should show:

- multiple distinct slots
- `TouchDown` for each active control
- no unexpected `TouchUp`

For mouse-look mode tests, the trace should show:

- no look touch while disabled
- look touch starts when enabled
- look touch ends immediately when disabled

## 9. Failure Mapping

### Daemon Never Connects To Android Server

Likely causes:

- Waydroid session not running
- container frozen
- wrong jar path
- server launch failure
- wrong container IP or port

Check:

- `waydroid status`
- Phantom daemon logs
- `/data/local/tmp/phantom-server.log`

### Server Starts But Game Does Not React

Likely causes:

- profile mismatch
- wrong screen contract
- wrong region or position placement
- game-specific behavior

Check:

- `phantom audit`
- profile screen vs daemon screen
- visible Android touch feedback if enabled

### Mouse Look Feels Broken

Likely causes:

- wrong activation mode
- wrong activation key
- mouse routing disabled
- region too small or misplaced
- sensitivity too low or too high

Check:

- GUI properties panel
- `phantom status`
- trace logs for `TouchDown`, `TouchMove`, `TouchUp`

### Touch Gets Stuck

Likely causes:

- profile bug
- runtime transition not releasing state
- crash during active touch sequence

Recovery:

```bash
./target/release/phantom pause
./target/release/phantom resume
./target/release/phantom exit-capture
./target/release/phantom enter-capture
```

If needed, restart the daemon.

## 10. `uinput` Appendix

Use this only if you intentionally test the legacy backend.

Recommended order:

```bash
sudo ./target/release/phantom --trace --daemon
waydroid session stop
waydroid session start
```

Checks:

```bash
grep -A5 "Phantom Virtual Touch" /proc/bus/input/devices
waydroid shell getevent -lp | grep -A10 "Phantom"
waydroid shell dumpsys input | grep -A20 "Phantom"
```

Those checks do not apply to `android_socket`.
