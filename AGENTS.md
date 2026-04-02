# AGENTS

This file defines the preferred contribution loop for both human contributors and automated coding agents working on Phantom.

The goal is simple:

- understand the current behavior first
- make the smallest correct change
- verify it
- leave the repository easier to debug than you found it

## Project Priorities

When making decisions, prefer:

1. correct runtime behavior
2. deterministic profiles and explicit state
3. maintainability and debuggability
4. polish and convenience

Do not trade the first three away for the fourth.

## Default Contribution Loop

Use this loop from start to finish.

### 1. Read Before Changing

At minimum, inspect:

- [README.md](README.md)
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- [docs/PROFILES.md](docs/PROFILES.md)
- the directly affected Rust files

If the task is game-setup related, also read:

- [docs/GAME_PATTERNS.md](docs/GAME_PATTERNS.md)
- [docs/TROUBLESHOOT.md](docs/TROUBLESHOOT.md)

### 2. Find The Real Layer

Decide where the problem actually belongs:

- input discovery/translation: `phantom/src/input.rs`
- engine semantics: `phantom/src/engine.rs`
- backend injection/runtime transport: `phantom/src/android_inject.rs`, `phantom/src/inject.rs`, `phantom/src/touch.rs`
- daemon orchestration/IPC/CLI: `phantom/src/main.rs`, `phantom/src/ipc.rs`, `phantom/src/overlay.rs`
- profile schema and audits: `phantom/src/profile.rs`
- GUI/editor behavior: `phantom-gui/src/main.rs`, `phantom-gui/src/overlay.rs`
- docs and operator contract: `README.md`, `docs/`

Fix the actual layer. Do not push engine bugs into the GUI, and do not hide runtime defects with docs-only changes.

### 3. Change The Smallest Surface That Solves It

Preferred approach:

- preserve existing architecture
- make one coherent change at a time
- avoid speculative refactors unless they are required to fix the issue correctly

Examples:

- if a hotkey works but logs are unusable, tighten logging rather than rewriting the event loop
- if a profile feature is missing from the GUI, add the editor support instead of inventing a hidden runtime default
- if an issue is compositor-specific, document the limit rather than pretending it is solved

### 4. Keep Behavior Explicit

Phantom should remain easy to reason about.

Good changes:

- explicit state transitions
- named modes
- audit output that reflects real runtime behavior
- logs that explain why something happened

Bad changes:

- hidden fallback behavior
- silent mutation of user profiles
- “best guess” magic without docs
- broad refactors with unclear product benefit

### 5. Verify The Right Things

Use the full standard verification set unless the change is clearly docs-only:

```bash
cargo fmt --all
cargo test --quiet
cargo clippy --quiet --all-targets --all-features -- -D warnings
cargo build --release --quiet
```

If the change touches runtime semantics, also validate against:

- [docs/TESTING.md](docs/TESTING.md)
- [docs/OPERATIONS.md](docs/OPERATIONS.md)

If the change touches profiles or setup workflows, confirm docs and examples remain aligned.

### 6. Update Docs As Part Of The Same Change

Update the relevant docs whenever you change:

- hotkeys
- install behavior
- runtime model
- profile schema
- backend behavior
- known limitations

Phantom treats documentation as part of the shipped product surface.

### 7. Commit Cleanly

Commits should be scoped and readable.

Use minimal prefixes:

- `feat:`
- `fix:`
- `docs:`
- `refactor:`
- `test:`
- `chore:`
- `release:`

Examples:

- `fix: propagate display env for overlay child`
- `docs: add shooter profile authoring patterns`
- `release: bump workspace to 0.6.1`

## Codebase-Specific Standards

### Engine

`phantom/src/engine.rs` is the semantic core.

Keep it:

- explicit
- synchronous
- testable
- free of platform I/O

### Input

`phantom/src/input.rs` is intentionally low-level.

Be careful with:

- evdev classification
- grab semantics
- touchpad versus mouse behavior
- `SYN_DROPPED` recovery
- noisy tracing in hot paths

### Overlay

The current host-side overlay is experimental.

Do not overstate it in code or docs. If behavior is compositor-dependent, say so clearly.

### Profiles

Profiles are a real compatibility surface.

When changing profile behavior:

- keep existing files valid when practical
- update validation and audit output
- update GUI editing support
- document the new semantics

## Logging Standard

Use logs intentionally.

Recommended levels:

- `info` for lifecycle and important runtime state changes
- `warn` for recoverable problems or degraded behavior
- `error` for failures that block the requested action
- `trace` for useful diagnosis
- raw/noisy per-device detail only behind explicit opt-in controls

For daemon tracing:

- `--trace` should remain usable
- `PHANTOM_TRACE_DETAIL=1` is the correct place for raw-device firehose logging

## Documentation Standard

The docs should be enough for:

- a user installing Phantom from scratch
- a maintainer debugging runtime issues
- a contributor adding a new feature
- an automated agent understanding expected behavior before modifying code

That means docs should describe:

- what exists
- how it behaves
- what is unsupported
- how to debug it

## What To Avoid

Do not:

- commit generated Android build artifacts
- keep stale design papers in the tracked repo root
- silently change operator workflows
- revert unrelated user work
- use destructive git commands unless explicitly requested

Local design notes belong in ignored workspace material such as `dev/`.

## When A Change Is Done

A change is done when:

- the bug or feature is actually addressed
- the affected docs match reality
- the relevant checks pass
- the resulting behavior is easier to understand than before

That is the contribution standard for Phantom.
