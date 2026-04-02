# Contributing

This project is maintained as a practical engineering tool, not a demo.

Contributions should preserve three qualities:

- deterministic runtime behavior
- explicit architecture and documentation
- fast debugging when something breaks in the field

If you are new to the repository, read these first:

1. [README.md](README.md)
2. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
3. [docs/PROFILES.md](docs/PROFILES.md)
4. [docs/GAME_PATTERNS.md](docs/GAME_PATTERNS.md)
5. [AGENTS.md](AGENTS.md)

## Getting Started

Clone the repository:

```bash
git clone https://github.com/oliviermugishak/phantom.git
cd phantom
```

Build the workspace:

```bash
cargo build --release
```

Build the Android server jar:

```bash
./contrib/android-server/build.sh
```

Run the standard verification set before submitting changes:

```bash
cargo fmt --all
cargo test --quiet
cargo clippy --quiet --all-targets --all-features -- -D warnings
cargo build --release --quiet
```

If your change touches runtime behavior, also validate it against the operational docs in [docs/TESTING.md](docs/TESTING.md).

## Development Model

The repository is split into three main contribution surfaces:

- `phantom/`
  The daemon, engine, input capture, backends, and IPC surface.
- `phantom-gui/`
  The native mapping editor and runtime control surface.
- `docs/`, `profiles/`, `config.example.toml`
  The operator and maintainer contract.

Use the right layer for the change:

- engine behavior belongs in `phantom/src/engine.rs`
- kernel/input translation belongs in `phantom/src/input.rs`
- backend injection belongs in `phantom/src/android_inject.rs`, `phantom/src/inject.rs`, or `phantom/src/touch.rs`
- profile schema belongs in `phantom/src/profile.rs`
- daemon orchestration and CLI behavior belong in `phantom/src/main.rs` and `phantom/src/ipc.rs`
- editor behavior belongs in `phantom-gui/src/main.rs`

## Code Standards

- Follow existing naming and file boundaries.
- Prefer explicit state transitions over hidden behavior.
- Keep comments rare and useful.
- Do not add “magic” behavior without documenting it.
- Preserve backward compatibility for saved profiles when practical.
- Update docs when runtime semantics, profile schema, hotkeys, or install behavior change.

## Testing Expectations

Minimum expected checks:

- formatting: `cargo fmt --all`
- tests: `cargo test --quiet`
- lints: `cargo clippy --quiet --all-targets --all-features -- -D warnings`

Add or update tests when you change:

- input translation
- engine semantics
- slot allocation behavior
- profile parsing or validation
- daemon runtime control behavior

If a change is hard to cover with automated tests, document the manual validation path in the relevant doc:

- [docs/TESTING.md](docs/TESTING.md)
- [docs/TROUBLESHOOT.md](docs/TROUBLESHOOT.md)
- [docs/GAME_PATTERNS.md](docs/GAME_PATTERNS.md)

## Documentation Expectations

Documentation is part of the product.

Update the relevant files when behavior changes:

- install and bootstrap: [docs/INSTALL.md](docs/INSTALL.md)
- runtime model and daily use: [docs/OPERATIONS.md](docs/OPERATIONS.md)
- profile schema and node behavior: [docs/PROFILES.md](docs/PROFILES.md)
- troubleshooting and known limits: [docs/TROUBLESHOOT.md](docs/TROUBLESHOOT.md)
- architectural rationale: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)

## Adding A New Control Type

Typical checklist:

1. Add the schema in `phantom/src/profile.rs`
2. Add state and runtime behavior in `phantom/src/engine.rs`
3. Add or update audits and validation
4. Add GUI creation and editing support in `phantom-gui/src/main.rs`
5. Add tests
6. Add docs and at least one example profile pattern if the new control is user-facing

## Commit Message Prefixes

Keep prefixes minimal and conventional:

- `feat:` new user-visible capability
- `fix:` bug fix or regression fix
- `docs:` documentation-only change
- `refactor:` internal restructuring without intended behavior change
- `test:` tests only
- `chore:` maintenance, tooling, or non-product housekeeping
- `release:` version bump or release packaging

Examples:

- `feat: add floating joystick zones`
- `fix: resync mouse look after re-grab`
- `docs: clarify overlay preview limitations`
- `release: bump workspace to 0.6.1`

## Pull Request Checklist

Before opening a PR or handing off a branch:

- the change is scoped cleanly
- the build is green
- docs match runtime behavior
- commit messages are readable
- no generated artifacts are accidentally tracked
- stale design notes are kept out of the tracked repo surface

## Need Deeper Workflow Guidance?

For the preferred contributor loop, change discipline, and agent-oriented working style, read [AGENTS.md](AGENTS.md).
