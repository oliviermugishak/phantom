# Troubleshooting And Limits

This document covers current operational limits and common failure cases.

## 1. Waydroid Is Running But Phantom Still Fails

Check the container state, not just the session state.

Bad:

- `Session: RUNNING`
- `Container: FROZEN`

Good:

- `Session: RUNNING`
- `Container: RUNNING`

For the Android backend, `FROZEN` is not good enough.

## 2. Android Server Times Out On Startup

Likely causes:

- Waydroid was not running before Phantom started
- the container was frozen
- the Android jar path is wrong
- the jar was not built as a dex jar
- `app_process` startup failed

Check:

```bash
sudo waydroid status
sudo waydroid shell -- sh -c 'tail -n 100 /data/local/tmp/phantom-server.log'
```

## 3. Wrong Touch Placement

Usually means the screen contract is wrong.

Check:

- daemon `screen`
- profile `screen`
- actual Waydroid surface size

Phantom is designed to fail fast on screen mismatch rather than silently stretch coordinates.

## 4. Mouse Look Is Not A Desktop Cursor

`mouse_camera` is a bounded swipe region with a short-lived synthetic finger.

It is good for:

- FPS camera
- action camera
- driving camera

It is not a general-purpose Android cursor substitute.

## 5. Mouse Look Feels Wrong

Check:

- region placement
- node sensitivity
- global sensitivity
- activation mode
- activation key
- mouse routing state

Good debugging path:

1. test `always_on`
2. confirm the region behaves
3. then add `while_held` or `toggle`

## 6. Stuck Touches

Recovery options:

```bash
./target/release/phantom pause
./target/release/phantom resume
./target/release/phantom exit-capture
./target/release/phantom enter-capture
```

If needed, restart the daemon.

## 7. Hotplug Rescan Is Not Implemented

Current limitation:

- Phantom does not dynamically rescan newly attached keyboards or mice after startup

If devices change, restart the daemon.

## 8. Floating Joysticks Are Not Supported

Current limitation:

- `joystick` is fixed-center

Profiles must be built for games and layouts that tolerate a fixed joystick anchor.

## 9. Multi-Monitor And Rotation Handling Are Not Supported

Current limitation:

- Phantom assumes one known target surface
- it does not manage monitor transforms or rotation transforms

Best practice:

- keep Waydroid fullscreen on one intended display
- use one stable screen contract

## 10. `uinput` Visibility Problems

This only applies to the compatibility backend.

If Phantom creates the virtual device but Waydroid does not react:

- restart Waydroid after starting Phantom
- verify `/dev/uinput` permissions
- verify the device appears in host input listings

This is one of the reasons the Android backend is now preferred.
