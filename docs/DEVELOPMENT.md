# Development

This document is the maintainer guide for Phantom.

It explains how to rebuild the project from scratch, how the repository is laid out, and where to make changes safely.

## Repository Map

Top-level structure:

- `phantom/`
  Rust daemon, engine, CLI, IPC, and backend code
- `phantom-gui/`
  Native editor and runtime control UI
- `profiles/`
  Shipped starter profile library
- `docs/`
  Product, runtime, profile, protocol, and maintainer docs
- `contrib/android-server/`
  Android-side `app_process` touch server
- `config.example.toml`
  Example runtime configuration

Important Rust files:

- `phantom/src/main.rs`
  CLI entrypoint, daemon startup, runtime shortcut handling, event loop
- `phantom/src/input.rs`
  Device discovery, `evdev` capture, grab state, and event translation
- `phantom/src/engine.rs`
  Profile-driven state machine that turns input into abstract touch commands
- `phantom/src/profile.rs`
  Profile schema, validation, and audit helpers
- `phantom/src/ipc.rs`
  JSON-over-Unix-socket control plane used by CLI and GUI
- `phantom/src/touch.rs`
  Touch backend trait
- `phantom/src/android_inject.rs`
  Android backend client transport
- `phantom/src/inject.rs`
  `uinput` backend
- `phantom/src/waydroid.rs`
  Waydroid discovery, container state handling, server staging, launch helpers

Important GUI file:

- `phantom-gui/src/main.rs`
  The editor currently lives in one main file. It owns canvas editing, bindings, runtime controls, and daemon polling.

Important Android server files:

- `contrib/android-server/build.sh`
  Java compile + dex build pipeline
- `contrib/android-server/src/com/phantom/server/PhantomServer.java`
  Android-side TCP server and input injection logic

## Full Rebuild From A Clean Machine

This is the maintainer rebuild flow.

### 1. Install Rust

Use `rustup` and then verify:

```bash
rustc --version
cargo --version
```

### 2. Install Android SDK Command-Line Tools

Phantom needs:

- one installed `android.jar`
- one installed `d8`

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

`build.sh` auto-detects the newest installed `android.jar` and `d8`.

### 3. Build The Rust Binaries

```bash
cargo build --release
```

Artifacts:

- `target/release/phantom`
- `target/release/phantom-gui`

### 4. Build The Android Server

```bash
./contrib/android-server/build.sh
```

Artifact:

- `contrib/android-server/build/phantom-server.jar`

The built jar must contain `classes.dex`. A plain `.class` jar is not valid for `app_process`.

### 5. Run The Test Suite

```bash
cargo test --quiet
```

### 6. Install Runtime Config

Recommended:

```bash
./install.sh
```

That is the supported maintainer install path for a user-local setup.

Manual alternative:

```bash
mkdir -p ~/.config/phantom/profiles
cp config.example.toml ~/.config/phantom/config.toml
cp profiles/*.json ~/.config/phantom/profiles/
```

## Versioning

Phantom now uses workspace-level versioning.

Version source of truth:

- root `Cargo.toml`
- `[workspace.package]`
- `version = "..."`

Both binaries inherit that shared version:

- `phantom`
- `phantom-gui`

That means:

- `phantom --version`
- `phantom version`
- `phantom-gui --version`
- `phantom-gui version`

all stay in sync with one version bump.

When making a release-style bump:

1. update the workspace version in the root `Cargo.toml`
2. run `cargo build --release`
3. run `./install.sh` if you want the installed binaries to match
4. confirm with `phantom --version` and `phantom-gui --version`

## Build Outputs

Rust outputs:

- daemon
- GUI

Android outputs:

- compiled classes in `contrib/android-server/build/classes/`
- dex jar in `contrib/android-server/build/phantom-server.jar`

## Maintainability Rules

These are the rules maintainers should preserve.

### Prefer Explicit State

Do not hide important runtime meaning in ad-hoc booleans scattered across files.

Examples of state that should stay explicit:

- capture enabled
- mouse routed
- keyboard routed
- engine paused
- active layers
- aim activation state

### Prefer Stable Contracts

Keep these contracts explicit:

- profile schema
- `TouchCommand` semantics
- Android wire protocol
- CLI and IPC requests
- runtime startup order

### Keep Android Backend First-Class

When touching backend behavior:

- treat `android_socket` as the primary path
- keep `uinput` working if practical
- do not force new features to be `uinput`-only unless absolutely necessary

### Update Docs With Behavior Changes

If any of these change, update docs in the same patch:

- startup order
- config shape
- runtime hotkeys
- profile schema
- Android protocol
- troubleshooting expectations

## Where To Change Things

### Add Or Change A Profile Node

Touch these places:

1. `phantom/src/profile.rs`
   - schema
   - validation
   - audit output
2. `phantom/src/engine.rs`
   - node state
   - input handling
   - release logic
   - tests
3. `phantom-gui/src/main.rs`
   - starter node creation
   - property editing
   - binding capture if relevant
   - canvas display
4. `docs/PROFILES.md`
5. `docs/ARCHITECTURE.md` if behavior meaning changed

### Change Runtime Controls

Touch these places:

1. `phantom/src/main.rs`
2. `phantom/src/ipc.rs`
3. `phantom/src/input.rs`
4. `phantom-gui/src/main.rs`
5. `config.example.toml`
6. `docs/OPERATIONS.md`
7. `docs/IPC.md`

### Change Android Transport

Touch these places:

1. `phantom/src/android_inject.rs`
2. `contrib/android-server/src/com/phantom/server/PhantomServer.java`
3. `docs/ANDROID_SOCKET_PROTOCOL.md`
4. `docs/ARCHITECTURE.md`
5. integration tests if behavior changed

## Code Comment Guidance

Add comments for:

- state-machine transitions that are not obvious
- backend assumptions that must stay synchronized
- runtime behavior that would be easy to misread later

Do not add comments that just restate the code literally.

## Safe Change Checklist

Before committing a behavioral change:

1. run `cargo fmt --all`
2. run `cargo test --quiet`
3. run `cargo build --release`
4. update docs if contracts changed
5. if the Android backend changed, verify the server still builds

## Release Mental Model

A Phantom release is healthy when:

- a new maintainer can rebuild it from a clean machine
- the architecture docs match the code
- the startup order is unambiguous
- a profile author can understand how slots and bindings work
- a runtime operator can diagnose failure without guessing
