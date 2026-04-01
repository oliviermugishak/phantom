# Troubleshoot

This document covers common operational problems and their direct fixes.

For product boundaries and missing features, see [EDGE_CASES.md](EDGE_CASES.md).

## GUI Does Not Show A New Shipped Profile

Symptom:

- a profile exists in the repository `profiles/` directory
- it does not appear in `phantom-gui`

Cause:

- the GUI reads `~/.config/phantom/profiles/`, not the repository directory

Fix:

```bash
./install.sh
phantom-gui
```

Why:

- `./install.sh` copies missing shipped profiles into the user profile library without overwriting existing user edits

## `F2` Works But `F1` Or `F8` Do Not

Symptom:

- shutdown works
- capture or mouse-toggle hotkeys do not

Likely cause:

- Fn Lock is off

Fix:

- enable Fn Lock so the top row emits real `F1`, `F8`, and `F9` function keys

## Waydroid Is Running But Phantom Still Cannot Connect

Check:

```bash
sudo waydroid status
```

Bad state:

- `Session: RUNNING`
- `Container: FROZEN`

Good state:

- `Session: RUNNING`
- `Container: RUNNING`

Fix:

```bash
waydroid show-full-ui
```

or open the target game first so the container wakes up fully.

## Android Server Auto-Launch Times Out

Check:

```bash
sudo waydroid shell -- sh -c 'tail -n 100 /data/local/tmp/phantom-server.log'
```

Common causes:

- Waydroid was not running before Phantom started
- container is frozen
- Android jar path is wrong
- the jar is not a dex jar

Verify the jar contains `classes.dex`.

## Touches Land In The Wrong Place

Cause:

- screen contract mismatch

Check:

- daemon screen in `phantom status`
- profile screen in `phantom audit`
- actual Waydroid surface size

Fix:

- align the profile and daemon screen contracts with the real fullscreen Android surface

## Mouse Look Does Not Work

Check:

- mouse routing is enabled
- capture is enabled
- `mouse_camera` region is correct
- `mouse_camera` activation mode matches the intended workflow
- activation key is present if mode is `while_held` or `toggle`

Useful cases:

- start with `always_on` to prove the region itself is correct
- then switch to `while_held` or `toggle`

## PUBG Sprint-Lock Drag Does Not Feel Right

Check:

- drag `start`
- drag `end`
- `duration_ms`

Fix:

- lower `duration_ms` for a faster snap
- adjust the drag end point to the exact sprint-lock point in your on-screen layout

## Temple Run Or Subway Surfers Swipe Feels Weak

Check:

- `duration_ms`
- swipe start point
- swipe end point

Fix:

- use a short drag duration, usually around `70-100 ms`
- move the swipe farther if the game wants a more deliberate gesture

## Temple Run Tilt Does Not Work

This is not a profile bug.

Reason:

- Phantom injects touch
- tilt is accelerometer input

Current status:

- tilt is unsupported

## New Profile Loads But The Game Still Ignores It

Check:

- the profile actually loaded in `phantom status`
- the profile screen matches the daemon screen
- the on-screen coordinates are from the same layout the game is currently using

If a layout was moved in-game, the profile must be updated too.

## Desktop Interaction Feels Broken After Starting The Daemon

Check:

- capture state
- mouse routing state

Use:

```bash
phantom exit-capture
phantom release-mouse
```

Remember:

- Phantom reserves daemon hotkeys
- gameplay capture should be the state that routes gameplay, not your normal desktop workflow
