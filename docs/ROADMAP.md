# Roadmap

This file tracks the highest-value next work after `0.6.1`.

It is intentionally short and product-oriented. It is not meant to be a complete issue tracker.

## 1. Runtime Efficiency

Status:

- partially addressed in `0.6.0`

Completed:

- GUI no longer repaints at a fixed `50ms` idle loop
- GUI runtime polling now uses slower intervals outside the Runtime tab
- overlay preview no longer repaints on a heartbeat while idle

Still worth doing:

- only poll fast while Runtime is open or shortly after a daemon action
- consider very slow polling when the GUI is unfocused
- add light instrumentation so GUI-side IPC frequency is measurable

## 2. Large-Profile Authoring

Status:

- still open

High-value items:

- control list search/filter by binding, id, type, and layer
- stronger bulk workflows for large layered profiles
- easier movement of controls across layers
- more guided flows for shooter-style profiles

## 3. Overlay Reliability

Status:

- open

High-value items:

- replace the current experimental host-side debug preview with an Android-side in-surface overlay
- keep improving diagnostics while the host overlay still exists
- document compositor/session expectations explicitly while the host overlay remains
- remove the host overlay entirely if it remains too unreliable to justify keeping

## 4. Gameplay Feel

Status:

- open

High-value items:

- continue touchpad aim tuning
- distinguish touchpad feel from real mouse feel more clearly
- keep improving resync behavior across capture and mouse-routing transitions
- tune fast-turn behavior for shooter use cases

## 5. Documentation Depth

Status:

- improved, still open

High-value items:

- extend game-pattern documentation further
- add more scenario-based JSON examples
- keep behavior docs aligned with runtime semantics
- add stronger “how to debug a bad profile” guidance
