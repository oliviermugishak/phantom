# Android Socket Protocol

This document describes the current host-to-Android wire protocol used by Phantom's primary backend.

Backend:

- `android_socket`

Transport:

- TCP

## 1. Transport Contract

Current defaults:

- host: Waydroid container IP discovered from `waydroid status`
- port: `27183`
- one long-lived client connection from the daemon to the Android server

Byte order:

- little-endian for integer payloads

## 2. Why This Protocol Exists

The Rust daemon and the Android server are intentionally split:

- host side owns capture and profile evaluation
- Android side owns MotionEvent injection

The protocol only carries finalized touch commands.

That keeps the host/container boundary simple.

## 3. Frame Types

### `TOUCH_DOWN`

- byte `0`: `0x00`
- byte `1`: slot `u8`
- bytes `2..6`: x `i32`
- bytes `6..10`: y `i32`

### `TOUCH_MOVE`

- byte `0`: `0x01`
- byte `1`: slot `u8`
- bytes `2..6`: x `i32`
- bytes `6..10`: y `i32`

### `TOUCH_UP`

- byte `0`: `0x02`
- byte `1`: slot `u8`

### `TOUCH_CANCEL`

- byte `0`: `0x03`
- byte `1`: slot `u8`

Current server behavior treats cancel as a full gesture cancel for that slot.

### `PING`

- request byte: `0x7f`
- response byte: `0x7f`

`PING` is the readiness check Phantom uses after launching the server.

## 4. Coordinate Contract

The engine produces normalized touch coordinates.

The Rust backend:

1. clamps normalized values to `[0.0, 1.0]`
2. scales them into the configured screen contract
3. writes integer pixel coordinates onto the wire

That means the Android server receives concrete pixel coordinates, not normalized floats.

## 5. Startup Lifecycle

Current happy path:

1. Waydroid session is already running
2. Waydroid container is unfrozen
3. Phantom stages `phantom-server.jar` into `/data/local/tmp/`
4. Phantom launches `app_process`
5. Android server binds `0.0.0.0:27183`
6. Rust client connects
7. Rust client sends `PING`
8. Android server replies `PING`
9. Normal touch traffic begins

## 6. Server Responsibilities

The Android server:

- accepts one client connection
- maintains touch pointer state
- reconstructs MotionEvents
- injects them through `InputManager.injectInputEvent()`

It is deliberately small. It is not trying to re-implement profile logic inside Android.

## 7. Health Signals

Healthy signals:

- daemon log shows connection success
- daemon log reports the touch backend ready
- server log shows startup
- server log shows client connected
- port `27183` is listening inside the container

Useful commands:

```bash
sudo waydroid shell -- sh -c 'tail -n 50 /data/local/tmp/phantom-server.log'
sudo waydroid shell -- sh -c 'ss -ltnp | grep 27183 || true'
```

## 8. Failure Notes

Common failures:

- Waydroid not running
- container frozen
- bad jar path
- server crash during `app_process` startup
- host connects to wrong IP or port

Current limitation:

- if the Android server dies after the daemon has connected, the daemon does not yet reconnect automatically

Restart the daemon in that situation.
