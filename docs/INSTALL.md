# Install

This is the clean-machine setup guide for Phantom.

It covers:

- Rust
- Android SDK tools
- Linux input access
- local install
- config creation
- shipped profile seeding
- first startup

The recommended backend is:

- `android_socket`

## 1. Prerequisites

You need:

- Linux
- Waydroid
- Rust toolchain
- Android SDK command-line tools
- access to `/dev/input/event*`

You only need `/dev/uinput` if you want the legacy `uinput` backend.

## 2. Clone The Repository

```bash
git clone https://github.com/oliviermugishak/phantom.git
cd phantom
```

## 3. Install Rust

Verify:

```bash
rustc --version
cargo --version
```

## 4. Install Android SDK Command-Line Tools

Phantom's Android backend build needs:

- `android.jar`
- `d8`

Suggested layout:

```text
~/Android/Sdk/
  cmdline-tools/latest/
  build-tools/<version>/
  platforms/android-<version>/
```

Recommended environment:

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

`contrib/android-server/build.sh` auto-detects the newest installed `android.jar` and `d8`.

## 5. Configure Linux Input Access

If you want to run the daemon as a normal user, configure device access:

```bash
sudo cp contrib/99-phantom.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -aG input "$USER"
```

Then log out and back in.

Verify:

```bash
groups | grep input
ls -l /dev/input/event*
```

If you always run the daemon with `sudo`, this is still recommended but less critical.

## 6. Optional: Enable `uinput`

Only do this if you want the legacy fallback backend:

```bash
sudo modprobe uinput
echo uinput | sudo tee /etc/modules-load.d/uinput.conf
ls -l /dev/uinput
```

## 7. Install Phantom

Recommended install:

```bash
./install.sh
```

That command:

- builds `phantom`
- builds `phantom-gui`
- builds the Android server jar
- installs binaries into `~/.local/bin`
- installs a sudo-visible `phantom` launcher into `/usr/local/bin` when possible
- installs `phantom-server.jar` into `~/.local/share/phantom/android/`
- creates `~/.config/phantom/config.toml` if missing
- refreshes `android.server_jar` in the existing config when it still points at a source-tree `contrib/android-server/build/phantom-server.jar`
- copies shipped profiles into `~/.config/phantom/profiles/` if those files do not already exist

Optional overwrite prompt:

- `./install.sh -o`
  interactively ask whether to overwrite the existing config and whether to overwrite the currently shipped profile filenames

Installed binaries:

- `phantom`
- `phantom-gui`

Important profile-library behavior:

- the GUI reads profiles from `~/.config/phantom/profiles/`
- the installer seeds that directory from the repository's `profiles/` directory
- rerunning `./install.sh` copies any newly added shipped profiles without overwriting your edited ones
- `./install.sh -o` can overwrite only the shipped filenames that currently exist in the repo
- `./install.sh -o` does not delete older extra profiles that are no longer shipped

Uninstall later with:

```bash
./install.sh -u
```

That removes installed binaries, the sudo-visible `phantom` launcher, and the installed Android server jar, but leaves your user config and profiles intact.

## 8. Manual Build

If you do not want to install yet:

```bash
cargo build --release
./contrib/android-server/build.sh
```

Artifacts:

- `target/release/phantom`
- `target/release/phantom-gui`
- `contrib/android-server/build/phantom-server.jar`

The Android jar must contain `classes.dex`.

## 9. Config

If you used `./install.sh`, `~/.config/phantom/config.toml` already exists.

If not:

```bash
mkdir -p ~/.config/phantom/profiles
cp config.example.toml ~/.config/phantom/config.toml
cp profiles/*.json ~/.config/phantom/profiles/
```

Minimum important fields:

```toml
log_level = "info"
touch_backend = "android_socket"

[screen]
width = 1920
height = 1080

[android]
auto_launch = true
# Optional explicit override. If missing or stale, Phantom also tries the
# installed jar under ~/.local/share/phantom/android/ and the current source
# tree build under contrib/android-server/build/phantom-server.jar.
server_jar = "/absolute/path/to/phantom-server.jar"
server_class = "com.phantom.server.PhantomServer"
container_bind_host = "0.0.0.0"
port = 27183
container_server_jar = "/data/local/tmp/phantom-server.jar"
container_log_path = "/data/local/tmp/phantom-server.log"

[waydroid]
work_dir = "/var/lib/waydroid"
```

Runtime hotkeys:

```toml
[runtime_hotkeys]
mouse_toggle = "F1"
capture_toggle = "F8"
pause_toggle = "F9"
overlay_toggle = "F10"
shutdown = "F2"
```

Keyboard note:

- on many laptops, `F1`, `F8`, and `F10` only arrive as real function keys when Fn Lock is enabled
- if `F2` works but `F1`, `F8`, or `F10` do not, check Fn Lock first

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
sudo phantom --trace --daemon
```

If you need raw device-level input tracing beyond the normal runtime trace:

```bash
sudo env PHANTOM_TRACE_DETAIL=1 phantom --trace --daemon
```

If the container is `FROZEN`, open the UI or game first.

## 11. Verify Bring-Up

In a normal user shell:

```bash
phantom status
phantom audit ~/.config/phantom/profiles/pubg.json
phantom load ~/.config/phantom/profiles/pubg.json
phantom-gui
```

Expected:

- daemon reachable
- GUI sees profiles from `~/.config/phantom/profiles/`
- `phantom-server.jar` path resolves correctly
- the Android backend connects cleanly

## 12. If A New Shipped Profile Does Not Appear

Do this:

```bash
./install.sh
phantom-gui
```

Reason:

- the GUI reads the user profile library
- the installer seeds new shipped profiles into that directory if they are missing

That is the supported sync model.
