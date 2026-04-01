# Android Touch Server

This directory contains the Android-side server used by Phantom's primary backend.

It is launched with:

- `app_process`

It receives:

- touch frames from the host daemon over TCP

It injects:

- Android `MotionEvent`s through `InputManager.injectInputEvent()`

## Why This Exists

The server exists so Phantom can keep:

- input capture
- profile logic
- control state

on the Linux host, while moving the final touch injection into Android itself.

That is the architecture the project is now built around.

## Build

Build with:

```bash
./contrib/android-server/build.sh
```

`build.sh` does:

1. find `android.jar`
2. compile Java sources
3. package classes
4. run `d8`
5. emit a dex jar suitable for `app_process`

Output:

- `contrib/android-server/build/phantom-server.jar`

If the jar does not contain `classes.dex`, it is not valid for this backend.

## SDK Detection

`build.sh` checks these in order:

1. `ANDROID_JAR`
2. `ANDROID_SDK_ROOT`
3. `ANDROID_HOME`
4. `~/Android/Sdk`

It uses the newest installed platform jar and build tools it can find.

## Runtime Integration

The daemon can auto-stage and auto-launch this server when configured:

```toml
touch_backend = "android_socket"

[android]
auto_launch = true
server_jar = "/absolute/path/to/ttplayer/contrib/android-server/build/phantom-server.jar"
```

Current runtime defaults:

- bind host: `0.0.0.0`
- port: `27183`
- staged jar path: `/data/local/tmp/phantom-server.jar`
- log path: `/data/local/tmp/phantom-server.log`

## Important Operational Rules

- Waydroid must already be running before the daemon starts
- the container must be `RUNNING`, not `FROZEN`
- if the server dies after the daemon connects, restart the daemon

## Manual Launch

Manual launch inside the container:

```bash
waydroid shell sh -c 'CLASSPATH=/data/local/tmp/phantom-server.jar app_process / com.phantom.server.PhantomServer --host 0.0.0.0 --port 27183'
```

Manual log check:

```bash
sudo waydroid shell -- sh -c 'tail -n 50 /data/local/tmp/phantom-server.log'
```

## Debug Signals

Good server signals:

- `phantom-server starting host=0.0.0.0 port=27183`
- `phantom-server client connected`

Bad signs:

- no log output
- listener exists but Phantom ping fails
- container still frozen
