# Contributing

## Code style

- No comments unless asked
- Follow existing conventions in the codebase
- Run `cargo fmt` before committing
- Run `cargo clippy` and fix all warnings
- Run `cargo test` — all tests must pass

## Project layout

```
phantom/
├── src/
│   ├── main.rs       — entry point, CLI parsing, daemon loop, signal handling
│   ├── engine.rs     — keymap state machine, all node type logic
│   ├── input.rs      — evdev device discovery, grab, event parsing
│   ├── inject.rs     — uinput device creation, MT Protocol B injection
│   ├── ipc.rs        — Unix socket server, IPC protocol, CLI client
│   ├── profile.rs    — JSON profile parsing, validation
│   ├── config.rs     — config.toml loading, path resolution
│   └── error.rs      — error types
├── tests/
│   └── integration.rs — integration tests
└── Cargo.toml
```

## Module responsibilities

### `engine.rs` — no I/O

The keymap engine is a pure state machine. It receives `InputEvent` and produces `Vec<TouchCommand>`. It does no file I/O, no system calls, no async. This makes it fully testable without hardware.

### `input.rs` — raw kernel interface

Opens `/dev/input/event*`, calls `EVIOCGRAB`, reads raw `input_event` structs via epoll. Converts evdev codes to our `Key` enum.

### `inject.rs` — raw kernel interface

Opens `/dev/uinput`, configures the virtual touchscreen via ioctls, writes `input_event` structs for MT Protocol B touch events.

### `ipc.rs` — async server

Tokio-based Unix socket server. Parses JSON requests, dispatches to daemon state, returns JSON responses.

### `profile.rs` — data only

Serde-based JSON parsing. Validates profiles on load (slot uniqueness, coordinate ranges, key validity). No runtime state.

## Adding a new node type

1. Add variant to `Node` enum in `profile.rs` (with `#[serde(tag = "type")]`)
2. Add variant to `NodeState` enum in `engine.rs`
3. Add initialization in `KeymapEngine::init_state()`
4. Add key press handling in `handle_key_press()`
5. Add key release handling in `handle_key_release()`
6. Add tick handling in `tick()` if time-based
7. Add release handling in `release_all()`
8. Add validation in `profile.rs::validate_node()`
9. Add GUI support in `phantom-gui/src/main.rs`
10. Add integration tests in `tests/integration.rs`

## Testing

```bash
# All tests
cargo test

# Daemon only
cargo test -p phantom

# Integration tests only
cargo test --test integration

# Include hardware tests (needs /dev/uinput)
sudo cargo test -- --ignored

# Specific test
cargo test joystick_wasd
```

## Commit messages

- `fix: description` — bug fixes
- `feat: description` — new features
- `refactor: description` — code restructuring
- `test: description` — test additions/changes
- `docs: description` — documentation changes

## Architecture decisions

Document significant decisions in `docs/`. If you change a fundamental design choice (e.g., switch from evdev grab to Wayland protocols), update `docs/ARCHITECTURE.md` with the rationale.
