# `uinput` Protocol Reference

This document is the legacy backend reference for Phantom's `uinput` path.

Read this only if you are:

- intentionally using `touch_backend = "uinput"`
- debugging low-level kernel touch behavior
- comparing the fallback backend against the primary Android backend

If you are using the current recommended architecture, start with:

- [ANDROID_SOCKET_PROTOCOL.md](ANDROID_SOCKET_PROTOCOL.md)
- [TESTING.md](TESTING.md)

## 1. Purpose

The `uinput` backend turns abstract `TouchCommand`s into Linux kernel multi-touch events.

That means Phantom is responsible for:

- slot selection
- tracking IDs
- touch-down/up sequencing
- final `SYN_REPORT` batching

This is exactly why the backend is now considered the fallback path for Waydroid.

## 2. Device Creation

Phantom opens `/dev/uinput` and creates a direct-touch virtual device.

Enabled capabilities include:

- `EV_ABS`
- `EV_KEY`
- `EV_SYN`
- `ABS_MT_SLOT`
- `ABS_MT_TRACKING_ID`
- `ABS_MT_POSITION_X`
- `ABS_MT_POSITION_Y`
- `ABS_MT_TOUCH_MAJOR`
- `ABS_MT_PRESSURE`
- `BTN_TOUCH`
- `INPUT_PROP_DIRECT`

## 3. Runtime Touch Model

Current model:

- 10 slots
- monotonic tracking IDs
- MT Protocol B
- batched writes ending in `SYN_REPORT`

## 4. Coordinate Conversion

Normalized coordinates are scaled into:

- `0..screen_width-1`
- `0..screen_height-1`

That scaling still depends on the same explicit screen contract as the Android backend.

## 5. Waydroid Implication

The virtual device exists on the host kernel side.

Whether Waydroid sees it immediately depends on:

- kernel device visibility
- Waydroid input discovery timing
- current session state

That is why the safe order for this backend is usually:

1. start Phantom
2. restart Waydroid

## 6. Important Caveat

This backend remains useful, but the project should not be re-centered around it unless there is a very good reason.
