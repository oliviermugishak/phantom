# Uinput And MT Protocol Reference

This document describes the kernel-facing contract Phantom relies on.

## Device Creation

Phantom opens `/dev/uinput` in nonblocking write mode and configures a direct-touch virtual device.

Enabled capabilities:

- `EV_ABS`
- `EV_KEY`
- `EV_SYN`
- `ABS_MT_SLOT`
- `ABS_MT_TRACKING_ID`
- `ABS_MT_POSITION_X`
- `ABS_MT_POSITION_Y`
- `BTN_TOUCH`
- `INPUT_PROP_DIRECT`

Preferred setup path:

1. `UI_DEV_SETUP`
2. `UI_ABS_SETUP` for each MT axis
3. `UI_DEV_CREATE`

Fallback path:

1. write `uinput_user_dev`
2. `UI_DEV_CREATE`

The modern path is used when supported. The fallback keeps the daemon usable on older kernels or older `uinput` implementations.

## Device Identity

Current identity:

- name: `Phantom Virtual Touch`
- bustype: `BUS_VIRTUAL`
- vendor: `0x1234`
- product: `0x5678`

This is suitable for input injection, but it is not an attempt to masquerade as undetectable physical hardware.

## Axis Ranges

Configured ranges:

- `ABS_MT_SLOT`: `0..9`
- `ABS_MT_TRACKING_ID`: `0..65535`
- `ABS_MT_POSITION_X`: `0..screen_width-1`
- `ABS_MT_POSITION_Y`: `0..screen_height-1`

The position ranges must match the touch surface Phantom is trying to emulate for Waydroid. If they do not, touches will be offset or scaled incorrectly.

## Runtime Touch Model

Phantom currently uses:

- MT Protocol B slots
- one tracking ID per active slot
- tracking ID equal to slot number
- `BTN_TOUCH = 1` when the first touch becomes active
- `BTN_TOUCH = 0` when the last touch is released

That tracking strategy is valid as long as a slot is not reused without first emitting `TRACKING_ID = -1`.

## Event Layout

`input_event` is treated as the 24-byte x86_64 kernel layout:

- `tv_sec: i64`
- `tv_usec: i64`
- `type_: u16`
- `code: u16`
- `value: i32`

Phantom writes zero timestamps and lets the kernel timestamp the event stream.

## Event Sequences

### First Finger Down

Example for slot `2` at `(960, 540)`:

```text
EV_ABS  ABS_MT_SLOT         2
EV_ABS  ABS_MT_TRACKING_ID  2
EV_ABS  ABS_MT_POSITION_X   960
EV_ABS  ABS_MT_POSITION_Y   540
EV_KEY  BTN_TOUCH           1        # only when this is the first active touch
EV_SYN  SYN_REPORT          0
```

### Finger Move

```text
EV_ABS  ABS_MT_SLOT         2
EV_ABS  ABS_MT_POSITION_X   980
EV_ABS  ABS_MT_POSITION_Y   530
EV_SYN  SYN_REPORT          0
```

### Finger Up

```text
EV_ABS  ABS_MT_SLOT         2
EV_ABS  ABS_MT_TRACKING_ID  -1
EV_KEY  BTN_TOUCH           0        # only when this was the last active touch
EV_SYN  SYN_REPORT          0
```

## Coordinate Conversion

Phantom clamps normalized coordinates to `[0.0, 1.0]`, multiplies by the configured screen size, then clamps again to `[0, max-1]`.

That means out-of-bounds profile math cannot emit invalid absolute coordinates to the kernel.

## Resolution Selection

Current daemon resolution order:

1. config override
2. default profile `screen`
3. `/sys/class/graphics/fb0/virtual_size`
4. `/dev/fb0`
5. fallback `1920x1080`

This is a host-side heuristic, not an Android-surface discovery API.

## Waydroid Implication

The virtual device is created on the host kernel and exposed through the shared input subsystem. Whether a running Waydroid session notices that device immediately depends on the active Waydroid/container setup. In practice, starting Phantom before the Waydroid session is the safest path.
