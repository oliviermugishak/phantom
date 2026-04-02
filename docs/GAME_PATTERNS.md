# Game Patterns

This guide explains how to structure Phantom profiles for real games, especially:

- fast-paced shooters such as PUBG Mobile and Call of Duty Mobile
- games with multiple contexts like vehicles, parachutes, loot, and menus
- swipe games and floating-stick games

The goal is not just to list node types. The goal is to show how to combine them into profiles that are understandable, testable, and maintainable.

## 1. Design Rule

Before placing controls, decide this first:

- which controls are always available
- which controls are only available in a special context
- which contexts are mutually exclusive

In Phantom terms, that becomes:

- base layer for always-available actions
- named layers for context-specific actions
- `layer_shift` nodes to enter and exit those contexts

If you do this up front, the profile stays readable even when it gets large.

## 2. Shooter Baseline

A practical shooter profile usually starts with a base layer that includes:

- one movement `joystick`
- one `mouse_camera`
- fire button
- ADS button
- jump
- crouch
- prone
- reload
- interact / pickup
- map
- inventory or backpack

For example:

- `joystick` on `W/A/S/D`
- `mouse_camera` on `MouseRight` in `while_held` or `toggle`
- `hold_tap` on `MouseLeft` for fire
- `tap` on `Space` for jump
- `tap` on `C` for crouch
- `tap` on `Z` for prone
- `tap` on `R` for reload
- `tap` on `F` or `E` for interact

That is enough for baseline combat before layers are introduced.

## 3. When To Use Layers

Use layers when a context changes the meaning of the keyboard.

Good layer candidates:

- `vehicle`
- `parachute`
- `loot`
- `map`
- `build`
- `spectate`

Do not create layers just because you can. Only create one when:

- a context has different bindings from the base layer
- the same physical key should do something different in that context
- the game presents a different control surface

## 4. Shooter Layer Strategy

### Base Layer

Keep these in base:

- movement
- look
- fire
- ADS
- jump
- crouch
- prone
- reload
- basic interact

Why:

- these are core moment-to-moment actions
- they should not depend on extra mode state

### Vehicle Layer

Put only true vehicle-only controls here:

- accelerate
- brake
- steer left / right
- nitro
- seat switch
- exit vehicle

Use a `layer_shift` that activates the `vehicle` layer:

- `hold` if vehicle mode should only exist while a key is held
- `toggle` if it should stay active until explicitly turned off

For PUBG-style vehicles, `toggle` is usually easier because entering a vehicle is a durable state.

### Parachute Layer

Use a separate `parachute` layer if the game changes the control surface in the air.

Typical parachute controls:

- dive
- glide
- free-look
- release or cut

This keeps parachute actions from cluttering base combat.

### Loot / Inventory Layer

This is usually best as a temporary `hold` layer.

Use it for:

- temporary remaps while looting
- special nearby interaction buttons
- inventory-only controls

Why `hold`:

- the user enters the mode temporarily
- the profile returns to normal automatically on release

## 5. Mouse Look Patterns

### `always_on`

Use when:

- capture should always steer the camera
- the game expects constant look control

Good for:

- some third-person or driving contexts

Risk:

- it makes desktop-style mouse behavior impossible while capture is active

### `while_held`

Use when:

- a key should both enable camera movement and enter a related gameplay mode

Good for:

- ADS on `MouseRight`
- scoped look
- temporary free-look

This is the best default for shooter profiles.

### `toggle`

Use when:

- the game benefits from explicit mouse-look on/off
- the player wants to switch between cursor-like interaction and camera look

Good for:

- games with frequent menu or cursor moments
- players who want a strong “locked look mode”

## 6. Sprint Lock With `drag`

Some games use a drag-up movement stick gesture to lock sprint.

That pattern maps well to Phantom:

- movement stays a `joystick`
- sprint lock becomes a one-shot `drag`

Recommended setup:

- `joystick` on `W/A/S/D`
- `drag` on `LeftShift`
- drag start near the movement-stick center
- drag end on the sprint-lock position
- short duration such as `70..120ms`

This keeps sprint lock separate from the movement stick itself.

## 7. Fixed vs Floating Joystick

Use `fixed` when:

- the game shows a visible static stick
- the movement origin should always be the same

Use `floating` when:

- the game accepts movement from a touch zone
- the origin can be chosen dynamically within a region

Practical examples:

- PUBG left stick: usually `fixed`
- football-style drag movement zone: usually `floating`
- some MOBAs with loose movement areas: usually `floating`

## 8. COD / PUBG Large-Profile Planning

For large shooter profiles, use this order:

1. build base combat first
2. validate movement + look + fire + ADS
3. add sprint lock if needed
4. add one secondary context layer at a time
5. validate each layer independently

Recommended layer order:

1. base combat
2. vehicle
3. parachute
4. loot / inventory

Do not build all layers at once. That makes debugging much harder.

## 9. JSON Example: Vehicle Layer

```json
{
  "id": "vehicle_layer",
  "type": "layer_shift",
  "key": "V",
  "layer_name": "vehicle",
  "mode": "toggle"
}
```

Then vehicle-only actions can live in:

```json
"layer": "vehicle"
```

For example:

- `tap` for exit
- `hold_tap` for accelerate
- `hold_tap` for brake
- `tap` for seat switch

## 10. JSON Example: ADS + Mouse Look

```json
{
  "id": "camera",
  "type": "mouse_camera",
  "slot": 2,
  "layer": "",
  "region": { "x": 0.35, "y": 0.0, "w": 0.65, "h": 1.0 },
  "sensitivity": 1.15,
  "activation_mode": "while_held",
  "activation_key": "MouseRight",
  "invert_y": false
}
```

This is the most useful default mouse-look pattern for shooters.

## 11. Testing Strategy For Large Profiles

Do not test a large shooter profile as one giant unit.

Test this way:

1. movement only
2. movement + look
3. movement + look + fire
4. movement + look + ADS
5. add one more action at a time
6. then test each named layer separately

If something breaks, roll back to the last stable combination and isolate the failing control.

## 12. Signs You Need A Layer

Add a layer when:

- the same key should mean different things in different game modes
- the game surface changes significantly
- base controls start feeling cluttered

Do not add a layer when:

- the action can simply live in base
- the context is brief and only needs one extra key
- you are trying to “organize visually” instead of modeling actual game state

## 13. Signs You Need A Different Primitive

Use a different control type instead of forcing one primitive to do everything:

- use `drag` for one-shot swipes
- use `joystick` for continuous movement
- use `mouse_camera` for camera/look
- use `hold_tap` for true held buttons
- use `toggle_tap` only when the game really wants a latched state

This keeps profiles clean and behavior predictable.
