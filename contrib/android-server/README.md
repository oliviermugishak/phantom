# Android Touch Server

This directory contains the Android-side `app_process` server used by the `android_socket` Phantom backend.

## Build

`build.sh` tries these in order:

1. `ANDROID_JAR` if explicitly set
2. `ANDROID_SDK_ROOT`
3. `ANDROID_HOME`
4. `~/Android/Sdk`

It compiles the server against `android.jar`, then runs `d8` to produce a dex jar suitable for `app_process`. A plain `.class` jar will not boot inside Waydroid.

It then picks the newest installed platform `android.jar`.

```bash
./contrib/android-server/build.sh
```

Output:

- `contrib/android-server/build/phantom-server.jar`

## Runtime

The Rust daemon can stage and launch this jar automatically when these config fields are set:

```toml
touch_backend = "android_socket"

[android]
auto_launch = true
server_jar = "/absolute/path/to/ttplayer/contrib/android-server/build/phantom-server.jar"
```

For the current implementation:

- Waydroid must already be running and unfrozen before the daemon starts
- the daemon may need elevated privileges to stage the jar into `/var/lib/waydroid/data/local/tmp`
- if the Android server dies after the daemon connects, restart the daemon

The daemon now copies the jar into the container over `waydroid shell` and talks to the server over TCP, not a shared filesystem socket.

The default runtime settings are:

- host: Waydroid container IP from `waydroid status`
- port: `27183`
- staged jar inside the container: `/data/local/tmp/phantom-server.jar`
- container log: `/data/local/tmp/phantom-server.log`

Important:

- `waydroid status` must show `Container: RUNNING`
- if it shows `Container: FROZEN`, open Waydroid with `waydroid show-full-ui` or launch the game first

Manual launch inside the Waydroid container:

```bash
waydroid shell sh -c 'CLASSPATH=/data/local/tmp/phantom-server.jar app_process / com.phantom.server.PhantomServer --host 0.0.0.0 --port 27183'
```

Log path inside the container:

```bash
sudo waydroid shell -- sh -c 'tail -n 50 /data/local/tmp/phantom-server.log'
```
