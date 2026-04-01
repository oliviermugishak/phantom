# Testing

This guide is the practical bring-up and validation checklist for Phantom.

The important split is backend choice:

- `uinput`: host-kernel virtual touchscreen
- `android_socket`: Android framework injection through `app_process`

Do not use the same verification signals for both. They are different pipelines.

## Backend Summary

| Backend | Start order | Main health signal | Main failure surface |
|---|---|---|---|
| `uinput` | start Phantom, then start or restart Waydroid | `getevent` / `dumpsys input` show Phantom device | Waydroid / kernel input bridge |
| `android_socket` | start Waydroid, make sure the container is unfrozen, then start Phantom | Phantom connects to `host:port` and server log shows client connected | Waydroid shell launch, container frozen state, framework injection |

## Common Preconditions

Before testing either backend:

1. Build the daemon:

```bash
cargo build --release -p phantom
```

2. Confirm your config has the real Android surface size:

```toml
[screen]
width = 1920
height = 1080
```

3. Confirm the daemon can read host input devices:

```bash
ls -l /dev/input/event*
```

4. Confirm your profile `screen` matches the daemon `screen`.

If the profile and daemon disagree, Phantom will reject the profile load.

5. Audit the profile once before runtime:

```bash
./target/release/phantom audit ~/.config/phantom/profiles/pubg.json
```

What to look for:

- the controls you want to hold simultaneously are on distinct `slot` lines
- layered controls are in the layer you expect
- joystick directions share one slot by design
- `mouse_camera` consumes one slot and is triggered by `MouseMove`

## `android_socket` Bring-Up

This is the recommended path for the Waydroid case where Phantom already proves it can hold multiple active slots, but Android still behaves as if only one touch exists.

### One-Time Build

The Android server jar is built from:

- [build.sh](/home/kaza/Workspace/ttplayer/contrib/android-server/build.sh)
- [PhantomServer.java](/home/kaza/Workspace/ttplayer/contrib/android-server/src/com/phantom/server/PhantomServer.java)

Build it:

```bash
./contrib/android-server/build.sh
```

Expected output:

- [phantom-server.jar](/home/kaza/Workspace/ttplayer/contrib/android-server/build/phantom-server.jar)

That jar must be a dex jar for `app_process`, not a plain `.class` jar. Phantom now checks this before auto-launch.

### Config

Minimal config:

```toml
touch_backend = "android_socket"

[screen]
width = 1920
height = 1080

[android]
auto_launch = true
server_jar = "/home/kaza/Workspace/ttplayer/contrib/android-server/build/phantom-server.jar"
server_class = "com.phantom.server.PhantomServer"
container_bind_host = "0.0.0.0"
port = 27183
container_server_jar = "/data/local/tmp/phantom-server.jar"
container_log_path = "/data/local/tmp/phantom-server.log"

[waydroid]
work_dir = "/var/lib/waydroid"
```

### Start Order

For `android_socket`, Waydroid must already be running and the container must be unfrozen before the daemon starts, because the daemon launches the Android server through `waydroid shell` and then waits for a live `PING` reply.

Bring-up sequence:

```bash
waydroid session start
waydroid show-full-ui
sudo ./target/release/phantom --trace --daemon
```

If `waydroid status` shows `Container: FROZEN`, Phantom will reject startup with a direct error telling you to open the Waydroid UI or launch the game first.

Because Phantom now resolves config and IPC paths from the invoking user, the daemon can be started with `sudo` while the normal user still uses:

```bash
./target/release/phantom status
./target/release/phantom load ~/.config/phantom/profiles/pubg.json
./target/release/phantom enter-capture
```

### Expected Startup Logs

On a healthy first start, expect some or all of:

- `touch backend ready` with `backend=android_socket`
- `android touch server unavailable, attempting auto-launch`
- `launching android touch server`
- `connected to android touch server`
- `IPC server listening on .../phantom.sock`

### Expected Files

Container-visible artifacts:

- staged server jar: `/data/local/tmp/phantom-server.jar`
- server log: `/data/local/tmp/phantom-server.log`
- TCP listener: `0.0.0.0:27183` by default

Useful checks:

```bash
sudo waydroid shell -- sh -c 'ls -l /data/local/tmp/phantom-server.jar'
sudo waydroid shell -- sh -c 'tail -n 50 /data/local/tmp/phantom-server.log'
sudo waydroid shell -- sh -c 'ss -ltnp | grep 27183 || true'
```

Expected server log lines:

- `phantom-server starting host=0.0.0.0 port=27183`
- `phantom-server client connected`

### What Not To Expect

For `android_socket`, these older checks are no longer authoritative:

- `getevent -lp` showing `Phantom Virtual Touch`
- `dumpsys input` showing a new touchscreen device
- IDC files affecting touch classification

Those belong to the `uinput` path. `android_socket` injects at the Android framework level, not through a new kernel input device.

## `android_socket` Test Matrix

Run these in order:

1. Single tap
2. Hold one control
3. Hold one control and tap another
4. Hold two controls at once
5. Double-tap style action
6. Joystick plus button
7. Mouse-look plus button
8. Layer switch or macro if your profile uses them

Suggested workflow:

1. Start Waydroid.
2. Start Phantom with `--trace`.
3. Audit the target profile.
4. Load the target profile.
5. Enter capture.
6. Open the target game.
7. Run the matrix above slowly first, then at game speed.

### Success Criteria

You want to see:

- the daemon receives the physical key events
- the daemon keeps emitting concurrent touch commands
- the game accepts the touches as separate simultaneous contacts

For example, when holding one button and pressing another, the daemon trace should show:

- both key events translated
- both events forwarded to the engine
- two `TouchDown` commands on different slots
- `active_touches=2`

And the game should continue to behave as if two fingers are down.

### Failure Signatures

If the daemon never reaches `connected to android touch server`:

- Waydroid session is not running
- Waydroid is running but `Container: FROZEN`
- `waydroid shell` failed
- the staged jar path is wrong
- the TCP host or port is wrong

If the daemon starts but the server log never shows `client connected`:

- the server failed during `app_process` startup
- the container never reached a responsive state
- the daemon is connecting to the wrong Waydroid IP or port

If the daemon connects and the server log is healthy but the game still ignores touches:

- the issue is inside Android/game dispatch, not the host evdev path
- inspect the exact action pattern being injected next

If the Android server dies after startup:

- restart the Phantom daemon
- current implementation does not yet reconnect to a restarted server mid-session

## `uinput` Bring-Up

Use this only if you want the host-kernel virtual touchscreen path.

Start order:

```bash
sudo ./target/release/phantom --trace --daemon
waydroid session stop
waydroid session start
```

Main checks:

```bash
grep -A5 "Phantom Virtual Touch" /proc/bus/input/devices
waydroid shell getevent -lp | grep -A10 "Phantom"
waydroid shell dumpsys input | grep -A20 "Phantom"
```

These checks are valid for `uinput`, not for `android_socket`.
