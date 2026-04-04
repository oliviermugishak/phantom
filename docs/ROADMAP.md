# Roadmap

This file tracks the highest-value next work after `0.8.0`.

It is intentionally short and product-oriented. It is not meant to be a complete issue tracker.

For the detailed architecture backlog, implementation boundaries, and feature
checklists, see [ROADMAD.md](ROADMAD.md).

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

## 2. Gameplay Control Primitives

Status:

- partially addressed in `0.8.x`

High-value items:

- macro improvements once the dedicated control primitives settle
- keep `aim` camera-first and keep touchpad behavior best-effort without penalizing real mouse feel

## 3. Large-Profile Authoring

Status:

- still open

High-value items:

- control list search/filter by binding, id, type, and layer
- stronger bulk workflows for large layered profiles
- easier movement of controls across layers
- more guided flows for shooter-style profiles

## 4. Overlay Reliability

Status:

- partially addressed

High-value items:

- owned `menu_touch` cursor overlay is now working and shipped
- the old `F10` host-side profile preview remains experimental/debug-only
- keep improving diagnostics while the host debug preview still exists
- document compositor/session expectations explicitly while the host debug preview remains
- replace the host debug preview with an Android-side in-surface overlay if that work is funded
- remove the host debug preview entirely if it remains too unreliable to justify keeping

## 5. Gameplay Feel

Status:

- partially addressed

High-value items:

- continue touchpad aim tuning
- distinguish touchpad feel from real mouse feel more clearly
- keep improving resync behavior across capture and mouse-routing transitions
- tune fast-turn behavior for shooter use cases
- keep owned `menu_touch` cursor feel direct without adding visible lag or smoothing

## 6. Documentation Depth

Status:

- improved, still open

High-value items:

- extend game-pattern documentation further
- add more scenario-based JSON examples
- keep behavior docs aligned with runtime semantics
- add stronger “how to debug a bad profile” guidance

## 7. Completed In 0.8.x

- owned `menu_touch` mode replaced the old released-mouse model
- menu-touch now keeps click ownership inside Phantom during capture
- working layer-shell menu-touch cursor overlay on Wayland/Hyprland
- accurate host-seeded menu-touch cursor initialization
- touchpad tap-to-click and double-tap-hold drag in owned menu-touch
- cursor overlay polish and direct cursor-feel refinements
- Aim V2 cleanup reduced the old region-tool feel and added explicit touchpad swipe re-arming for aim
- dedicated `wheel` controls now exist for paired scroll-up / scroll-down touch targets

## 8. Recommended Next Order

1. macro improvements
2. gameplay feel follow-up
3. large-profile authoring
4. runtime efficiency follow-through
5. documentation depth
6. host debug preview replacement or removal
