# phantom-gui

This crate is the native Phantom mapping studio.

It is not the runtime mapper itself. It is the editor and runtime control surface that sits on top of the daemon crate.

If you want the full product overview, start at [../README.md](../README.md). This document is specifically about the GUI crate.

## What This Crate Does

The GUI is responsible for:

- loading and saving profile JSON
- visual editing on a 16:9 canvas
- binding keys and mouse buttons
- creating and editing profile node types
- layer filtering and layered authoring flows
- runtime status display through IPC
- daemon control actions such as load, capture, and mouse routing
- experimental overlay preview rendering

Main files:

- `src/main.rs`
  The studio application, canvas, editor state, runtime actions, and persistence.
- `src/overlay.rs`
  Experimental host-side debug overlay preview mode.

## Studio Model

The GUI is intentionally a studio, not an in-game HUD.

It has four primary views:

- `Overview`
- `Inspect`
- `Runtime`
- `Settings`

Typical workflow:

1. open or create a profile
2. set the screen contract
3. place controls
4. convert or refine the selected control in `Inspect` when needed
5. bind keys
6. save
7. push live
8. switch to runtime controls when needed

## Persistence

The GUI persists studio preferences to:

- `~/.config/phantom/studio.toml`

That includes:

- label visibility
- snap setting
- auto-push preference
- last opened profile path
- current layer filter
- right-panel tab preference

Saved mappings live in the profile JSON files themselves.

The GUI does not try to persist every transient editor state. It persists the studio defaults that are useful across sessions and leaves actual profile data in the profile files.

## Profile Discovery

The GUI loads profiles from:

- `~/.config/phantom/profiles/`

It does not load the repository `profiles/` directory directly.

That is deliberate:

- repository profiles are shipped starter layouts
- user profiles are the working library

The supported sync path is `./install.sh`, which seeds missing profiles into the user library.

## Runtime Coupling

The GUI talks to the daemon through Phantom’s Unix socket IPC.

That means:

- the GUI can inspect runtime state
- the GUI can load profiles live
- the GUI can toggle capture and mouse routing
- the GUI does not execute gameplay mappings locally

The runtime authority remains the daemon in the `phantom` crate.

## Overlay Preview

The GUI also contains the experimental overlay preview mode used by `F10`.

Important:

- this is a host-side debug preview
- it is not a production in-game overlay
- compositor behavior can affect visibility and behavior

Treat it as a debugging aid, not a gameplay feature.

## Contribution Rules For This Crate

When changing the GUI:

- keep authoring behavior explicit
- avoid hiding runtime semantics behind editor-only magic
- preserve the separation between editing and runtime control
- keep large-profile workflows understandable
- update docs when node editing or runtime control semantics change

If you change editor behavior, also check:

- [../docs/PROFILES.md](../docs/PROFILES.md)
- [../docs/OPERATIONS.md](../docs/OPERATIONS.md)
- [../docs/GAME_PATTERNS.md](../docs/GAME_PATTERNS.md)

## Verification

Standard crate validation:

```bash
cargo clippy --quiet -p phantom-gui --all-targets --all-features -- -D warnings
cargo build --release -p phantom-gui --quiet
```

If the change affects runtime control or live editing flows, also run the full workspace checks.

For the broader contribution loop, see [../AGENTS.md](../AGENTS.md) and [../CONTRIBUTING.md](../CONTRIBUTING.md).
