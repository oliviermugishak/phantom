# Android Socket Protocol

Phantom's `android_socket` backend sends fixed-size binary frames to the Android-side `app_process` server.

## Transport

- TCP
- default host: Waydroid container IP from `waydroid status`
- default port: `27183`
- little-endian integer payloads
- one long-lived client connection from the Phantom daemon

## Frames

### `TOUCH_DOWN`

- byte `0`: `0x00`
- byte `1`: slot `u8`
- bytes `2..6`: `x` as `i32`
- bytes `6..10`: `y` as `i32`

### `TOUCH_MOVE`

- byte `0`: `0x01`
- byte `1`: slot `u8`
- bytes `2..6`: `x` as `i32`
- bytes `6..10`: `y` as `i32`

### `TOUCH_UP`

- byte `0`: `0x02`
- byte `1`: slot `u8`

### `TOUCH_CANCEL`

- byte `0`: `0x03`
- byte `1`: slot `u8`

Current server behavior treats cancel as a full gesture cancel.

### `PING`

- request byte: `0x7f`
- response byte: `0x7f`

## Coordinates

The Rust client scales normalized `TouchCommand` coordinates into integer pixel coordinates using the daemon's configured screen contract before writing them onto the wire.

## Lifecycle

Current startup flow:

1. Waydroid session is already running
2. Waydroid container is unfrozen
3. Phantom stages `phantom-server.jar` into `/data/local/tmp/`
4. Phantom launches `app_process`
5. Android server binds `0.0.0.0:27183` by default
6. Rust client connects and sends `PING`
7. Server replies `PING`

If `waydroid status` shows `Container: FROZEN`, the listener may exist but still fail the readiness ping until the UI is opened.

Health indicators:

- daemon log shows successful TCP connection
- daemon log shows `touch backend ready`
- server log shows client connected
- `ss -ltnp | grep 27183` inside the container shows the listener

Current limitation:

- if the Android server exits after connection, the daemon does not yet reconnect automatically; restart the daemon
