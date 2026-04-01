# Install And Rebuild

This is the complete setup guide for Phantom on a clean machine.

It covers:

- Rust toolchain
- Android SDK setup for the `app_process` server
- Linux input permissions
- build outputs
- config
- first startup

The recommended backend is `android_socket`.

## 1. Prerequisites

You need:

- Linux
- Waydroid
- Rust toolchain
- access to `/dev/input/event*`
- Android SDK command-line tools with:
  - at least one installed `android.jar`
  - at least one installed `d8`

You only need `/dev/uinput` if you intend to use the legacy `uinput` backend.

## 2. Clone The Repository

```bash
git clone <repo-url>
cd ttplayer
```

## 3. Install Rust

Verify:

```bash
rustc --version
cargo --version
```

## 4. Install Android SDK Command-Line Tools

Phantom's Android backend is built locally. The build needs:

- `android.jar`
- `d8`

Suggested SDK layout:

```text
~/Android/Sdk/
  cmdline-tools/latest/
  build-tools/<version>/
  platforms/android-<version>/
```

Recommended environment variables:

```bash
export ANDROID_HOME="$HOME/Android/Sdk"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
```

Install at least:

- one platform package
- one build-tools package

Example:

```bash
sdkmanager "platform-tools" "platforms;android-37" "build-tools;37.0.0"
```

The exact version numbers are not hard-coded into Phantom. `contrib/android-server/build.sh` auto-detects the newest installed `android.jar` and `d8`.

## 5. Configure Linux Input Access

If you will run the daemon as your normal user, add input access:

```bash
sudo cp contrib/99-phantom.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -aG input "$USER"
```

Then log out and log back in.

Verify:

```bash
groups | grep input
ls -l /dev/input/event*
```

If you plan to run the daemon with `sudo`, this step is less important but still recommended.

## 6. Optional: Enable `uinput`

Only do this if you plan to use the legacy `uinput` backend.

```bash
sudo modprobe uinput
echo uinput | sudo tee /etc/modules-load.d/uinput.conf
ls -l /dev/uinput
```

## 7. Build The Project

Build the Rust binaries:

```bash
cargo build --release
```

Artifacts:

- `target/release/phantom`
- `target/release/phantom-gui`

Build the Android server:

```bash
./contrib/android-server/build.sh
```

Artifact:

- `contrib/android-server/build/phantom-server.jar`

That jar must contain `classes.dex`.

## 8. Install Config And Profiles

```bash
mkdir -p ~/.config/phantom/profiles
cp config.example.toml ~/.config/phantom/config.toml
cp profiles/*.json ~/.config/phantom/profiles/
```

If you want a default profile:

```bash
cp profiles/pubg.json ~/.config/phantom/profiles/default.json
```

## 9. Edit `config.toml`

The minimum required fields are:

```toml
log_level = "info"
touch_backend = "android_socket"

[screen]
width = 1920
height = 1080

[android]
auto_launch = true
server_jar = "/absolute/path/to/ttplayer/contrib/android-server/build/phantom-server.jar"
server_class = "com.phantom.server.PhantomServer"
container_bind_host = "0.0.0.0"
port = 27183
container_server_jar = "/data/local/tmp/phantom-server.jar"
container_log_path = "/data/local/tmp/phantom-server.log"

[waydroid]
work_dir = "/var/lib/waydroid"
```

Also set your daemon hotkeys explicitly:

```toml
[runtime_hotkeys]
mouse_toggle = "F1"
capture_toggle = "F8"
pause_toggle = "F9"
shutdown = "F2"
```

## 10. First Startup

Start Waydroid first:

```bash
waydroid session start
waydroid show-full-ui
sudo waydroid status
```

Before starting Phantom, confirm:

- `Session: RUNNING`
- `Container: RUNNING`

Then start Phantom:

```bash
sudo ./target/release/phantom --trace --daemon
```

If the container is `FROZEN`, the Android server may listen but fail readiness checks. Open the UI or the game first.

## 11. Verify The Daemon

In a normal user shell:

```bash
./target/release/phantom status
```

Expected:

- daemon reachable
- screen contract visible
- capture state visible
- mouse routed / keyboard routed visible

Audit a profile:

```bash
./target/release/phantom audit ~/.config/phantom/profiles/pubg.json
```

Load a profile:

```bash
./target/release/phantom load ~/.config/phantom/profiles/pubg.json
```

Enter capture:

```bash
./target/release/phantom enter-capture
```

## 12. Launch The GUI

```bash
./target/release/phantom-gui
```

Recommended editor flow:

1. open the target profile
2. confirm the screen contract
3. place or edit controls
4. bind real keys
5. `Push Live`
6. test in-game

## 13. Android Backend-Specific Checks

Useful checks:

```bash
sudo waydroid shell -- sh -c 'ls -l /data/local/tmp/phantom-server.jar'
sudo waydroid shell -- sh -c 'tail -n 50 /data/local/tmp/phantom-server.log'
sudo waydroid shell -- sh -c 'ss -ltnp | grep 27183 || true'
```

Expected signs:

- server jar staged inside the container
- server log shows startup
- port `27183` listening
- daemon log shows successful connection

## 14. `uinput` Fallback Setup

If you intentionally want the compatibility backend:

```toml
touch_backend = "uinput"
```

Recommended startup order for that backend:

```bash
sudo ./target/release/phantom --trace --daemon
waydroid session stop
waydroid session start
```

That path is documented for compatibility only. The project is centered on `android_socket`.

## 15. Rebuild Checklist

Whenever you update the project:

```bash
cargo fmt --all
cargo test --quiet
cargo build --release
./contrib/android-server/build.sh
```

If the behavior or contracts changed, update docs in the same change.
