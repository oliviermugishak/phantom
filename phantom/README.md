# phantom

This crate is the Phantom daemon and runtime core.

It owns:

- CLI entry points
- daemon startup and shutdown
- `evdev` input capture
- runtime hotkeys
- profile execution
- touch backend dispatch
- IPC for the CLI and GUI

If you want the full product overview, start at [../README.md](../README.md). This document is specifically about the daemon crate.

## What This Crate Contains

Main surfaces:

- `src/main.rs`
  CLI parsing, logging setup, daemon bootstrap, event loop, runtime hotkeys.
- `src/input.rs`
  Linux `evdev` discovery, classification, grab state, and input translation.
- `src/engine.rs`
  Pure profile state machine that turns `InputEvent` into `TouchCommand`.
- `src/profile.rs`
  JSON profile schema, validation, and audit output.
- `src/ipc.rs`
  Unix socket control plane shared by the CLI and GUI.
- `src/touch.rs`
  Abstract touch backend creation and selection.
- `src/android_inject.rs`
  Primary Android socket backend.
- `src/inject.rs`
  Legacy `uinput` fallback backend.
- `src/waydroid.rs`
  Waydroid-specific startup and readiness checks.
- `src/overlay.rs`
  Experimental host-side overlay launcher.
- `src/logging.rs`
  Logging helpers, including raw trace detail gating.
- `tests/integration.rs`
  Integration coverage for runtime behavior.

## Runtime Architecture

The daemon pipeline is:

1. capture host input through `evdev`
2. translate raw kernel input into Phantom `InputEvent`
3. execute the loaded profile in the engine
4. emit `TouchCommand`
5. apply those commands through the selected touch backend

Simplified flow:

```text
evdev -> InputEvent -> KeymapEngine -> TouchCommand -> backend
```

Primary backend:

- `android_socket`

Fallback backend:

- `uinput`

The engine is intentionally synchronous and deterministic. Platform I/O belongs outside it.

## Runtime State Model

The daemon treats these as separate runtime states:

- daemon running
- capture active
- mouse routed
- keyboard routed
- engine paused
- active layers

This separation is important:

- capture decides whether gameplay input reaches the engine
- mouse routing decides whether mouse-originated gameplay input is routed
- pause freezes touch output without tearing down the daemon

## Logging

Normal operation:

```bash
sudo phantom --daemon
```

Useful runtime diagnosis:

```bash
sudo phantom --trace --daemon
```

Raw low-level device tracing:

```bash
sudo env PHANTOM_TRACE_DETAIL=1 phantom --trace --daemon
```

`--trace` should stay readable. `PHANTOM_TRACE_DETAIL=1` is reserved for raw `evdev` and similar high-noise diagnostics.

## Contribution Rules For This Crate

Keep module boundaries clean:

- `engine.rs` should stay testable and free of platform I/O
- `input.rs` should stay low-level and explicit
- `main.rs` should orchestrate, not absorb business logic
- `profile.rs` is part of the user-facing compatibility surface

If runtime semantics change, also update:

- [../docs/OPERATIONS.md](../docs/OPERATIONS.md)
- [../docs/PROFILES.md](../docs/PROFILES.md)
- [../docs/TESTING.md](../docs/TESTING.md)
- [../docs/TROUBLESHOOT.md](../docs/TROUBLESHOOT.md)

## Verification

Standard crate validation:

```bash
cargo test -p phantom --quiet
cargo clippy --quiet -p phantom --all-targets --all-features -- -D warnings
cargo build --release -p phantom --quiet
```

For the broader contribution workflow, see [../AGENTS.md](../AGENTS.md) and [../CONTRIBUTING.md](../CONTRIBUTING.md).
