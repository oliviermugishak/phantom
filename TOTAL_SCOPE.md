# Total Scope

This file is the product charter for Phantom as it exists now.

## Product Definition

Phantom is a Waydroid-focused fullscreen game mapper.

It maps Linux keyboard and mouse input into Android multi-touch gestures for one known Android surface size.

## Primary Architecture

Primary path:

- host `evdev` capture
- host profile engine
- Android-side touch injection through `app_process`

Compatibility path:

- host `uinput` virtual touchscreen injection

The primary path is the one maintainers should optimize, document, and extend first.

## Supported Scope

Implemented and supported:

- one local machine
- one fullscreen Waydroid target
- explicit screen contract
- live daemon control
- live GUI profile editing
- Android framework touch injection
- fallback `uinput` injection

Supported mapping primitives:

- `tap`
- `hold_tap`
- `toggle_tap`
- `joystick`
- `mouse_camera`
- `repeat_tap`
- `macro`
- `layer_shift`

## Deliberately Out Of Scope

Not product goals:

- dynamic UI recognition
- floating joystick discovery
- generic Android automation
- cursor-level desktop remapping
- multi-monitor correctness
- orientation-aware transformation logic
- compositor plugins

## Quality Bar

The project should prefer:

- deterministic behavior over heuristics
- explicit config over implicit guessing
- maintainable architecture over hacks
- good docs over tribal knowledge
- debuggable runtime state over hidden magic

## Documentation Requirement

The codebase is not considered maintainable unless these stay current:

- top-level README
- install and rebuild path from a clean machine
- architecture and design decisions
- operations and test flow
- profile schema
- backend protocol reference

## Near-Term Direction

The next layers of value are:

- better gameplay workflows
- stronger templates
- clearer runtime state
- better testing and diagnostics
- cleaner maintainer docs

The core architecture is no longer the blocker. Maintainability, usability, and profile quality are now the center of work.
