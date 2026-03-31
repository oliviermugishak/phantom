# Build Phases — Implementation Roadmap

Each phase produces a testable artifact. No phase depends on untested code from a later phase.

---

## Phase 0: Skeleton + Error Handling

**Goal:** Project compiles, runs, logs, exits cleanly on Ctrl+C.

**Deliverables:**
- `Cargo.toml` with all dependencies
- `src/main.rs` — entry point, CLI arg parsing (`--daemon`, `load`, `status`, etc.)
- `src/error.rs` — unified error types with `thiserror`
- `src/ipc.rs` — Unix socket server (accept, read JSON, write JSON, close)
- Logging with `tracing` — stdout for foreground, optional file
- Signal handler: `SIGTERM` and `SIGINT` set atomic shutdown flag
- Config loading: `~/.config/phantom/config.toml` (screen resolution override, log level)

**Test:** `cargo run -- --daemon` starts, prints log messages, Ctrl+C exits cleanly with "shutting down" log.

**Key decisions finalized here:**
- `nix` crate version and feature flags
- `tokio` feature set
- Error propagation strategy (anyhow for daemon, thiserror for library code)

---

## Phase 1: uinput Touch Injector

**Goal:** Create a virtual touchscreen and inject a test tap. Verify with `evtest`.

**Deliverables:**
- `src/inject.rs`:
  - `UinputDevice::new(width, height) -> Result<Self>`
  - `UinputDevice::touch_down(slot, x, y)`
  - `UinputDevice::touch_move(slot, x, y)`
  - `UinputDevice::touch_up(slot)`
  - `UinputDevice::destroy()`
  - Signal-safe cleanup (Drop impl)
- Test binary (or integration test): creates device, taps (500, 500), waits 1 second, lifts finger

**Verification:**
1. Run `evtest` on the created device — see correct MT events
2. Open Waydroid with "Show taps" enabled in developer options — see tap at correct position
3. Open Android's "Pointer Location" — verify coordinates match

**This is the critical proof-of-concept. If this doesn't work, the project doesn't work.**

**Edge cases tested:**
- Screen resolution detection from `/sys/class/graphics/fb0`
- Fallback to config if sysfs unavailable
- Device teardown on SIGTERM (verify no orphaned device)
- Rapid tap sequences (stress test)

---

## Phase 2: Input Capture + Keymap Engine (tap only)

**Goal:** Grab keyboard, press a key, see a touch event in Waydroid.

**Deliverables:**
- `src/input.rs`:
  - `discover_devices() -> Vec<InputDevice>`
  - `InputDevice::grab()`
  - `InputDevice::read_events() -> Vec<RawEvent>`
  - Device classification (keyboard vs mouse)
- `src/engine.rs`:
  - `KeymapEngine::new(profile) -> Self`
  - `KeymapEngine::process(event) -> Vec<TouchCommand>`
  - `tap` node type implementation
  - Key state tracking (ignore repeats)
  - Coordinate conversion (relative → pixel)
- `src/profile.rs`:
  - JSON parsing with `serde`
  - Validation (slot uniqueness, key validity, coordinate range)
- `src/main.rs`: event loop connecting input → engine → inject

**Verification:**
1. Load a profile with a single `tap` node bound to key `A` at position (0.5, 0.5)
2. Press `A` — touch appears at screen center in Waydroid
3. Release `A` — touch lifts
4. Verify key repeat is filtered (hold `A` — only one touch down, not repeated)

**Edge cases tested:**
- Multiple keyboards (events from both should work)
- Key repeat filtering
- SYN_DROPPED handling
- Invalid profile (malformed JSON, duplicate slots, bad coordinates)

---

## Phase 3: Joystick Node

**Goal:** WASD movement working in a game.

**Deliverables:**
- `joystick` node implementation in `engine.rs`:
  - 4-direction state tracking
  - Finger down at center on first key press
  - Continuous move events on direction change
  - Finger up on all keys released
  - Diagonal support (W+D = up-right)
- Integration with existing event loop

**Verification:**
1. Load profile with joystick node at (0.18, 0.72), radius 0.07
2. Open game (PUBG/COD Mobile)
3. Press W — character moves forward
4. Press W+D — character moves forward-right
5. Release all — character stops
6. Verify joystick doesn't "float" (touch always starts at center)

**Edge cases tested:**
- Rapid direction changes (W → W+D → D → S+D → S)
- All 4 keys pressed simultaneously (should normalize to center or ignore)
- Key held while daemon starts (should not activate until next press)

---

## Phase 4: Mouse Camera

**Goal:** Mouse look working in a game.

**Deliverables:**
- Mouse device detection and grab in `input.rs`
- Raw `EV_REL` delta computation (no Wayland protocols)
- `mouse_camera` node implementation in `engine.rs`:
  - Persistent finger (never lifts during game mode)
  - Delta accumulation with sensitivity
  - Region clamping
  - `invert_y` support
- Sensitivity multiplier from profile + runtime override

**Verification:**
1. Load profile with mouse_camera node
2. Move mouse — camera in game rotates
3. Verify smooth movement, no jitter
4. Adjust sensitivity — verify proportional change
5. Test with high-DPI mouse (set low sensitivity)

**Edge cases tested:**
- Mouse moves while no profile loaded (should be ignored)
- Very large single delta (mouse slam) — should clamp, not teleport
- Mouse device disappears mid-game — camera stops, other nodes continue

---

## Phase 5: hold_tap + repeat_tap Nodes

**Goal:** Fire button and auto-fire working.

**Deliverables:**
- `hold_tap` node: touch down on key press, touch up on key release
- `repeat_tap` node: timer-based repeated taps while key held
  - Configurable interval
  - Minimum 16ms interval enforcement
  - Clean stop on key release (no leftover finger)

**Verification:**
1. Bind fire button to `hold_tap` — hold to fire, release to stop
2. Bind auto-fire to `repeat_tap` — hold for rapid fire
3. Verify timing accuracy with `evtest` (check event timestamps)

**Edge cases tested:**
- Repeat interval smaller than event processing time (should not crash, just skip)
- Key released mid-tap (between TouchDown and scheduled TouchUp) — must lift finger
- Multiple repeat_tap nodes active simultaneously

---

## Phase 6: Daemon Polish + CLI

**Goal:** Production-ready daemon with full IPC and CLI.

**Deliverables:**
- Complete IPC command set (all commands from [IPC.md](./IPC.md))
- CLI tool (`phantom` binary without `--daemon` acts as client)
- Profile list/load/reload via CLI
- Daemon status reporting
- `config.toml` support
- Default profile auto-load from `~/.config/phantom/profiles/default.json`
- Proper logging (structured, with levels)
- Man page or `--help` output

**Verification:**
1. `sudo phantom --daemon` — starts cleanly
2. `phantom status` — shows running state
3. `phantom load ~/.config/phantom/profiles/pubg.json` — loads profile
4. `phantom reload` — reloads from disk
5. `phantom pause` / `phantom resume` — toggles input
6. `phantom shutdown` — clean exit

**Edge cases tested:**
- Start daemon twice (second instance detects first and exits)
- Stale socket cleanup
- Invalid CLI arguments
- IPC timeout (slow daemon response)

---

## Phase 7: GUI

**Goal:** Visual profile editor.

**Deliverables:**
- `phantom-gui/` workspace member
- egui application with:
  - Canvas displaying screenshot as background
  - Draggable node markers (colored circles with labels)
  - Node property panel (edit position, keys, slot, sensitivity)
  - Add/remove nodes
  - Save/Load profile buttons
  - Connect to daemon via IPC for live reload
- Screenshot loading (file dialog, or `adb exec-out screencap` integration)
- Coordinate display (show relative 0.0–1.0 and pixel values)

**Verification:**
1. Open GUI, load screenshot
2. Add tap node, drag to button position
3. Save profile
4. In Waydroid, press the bound key — touch at correct position
5. Edit node position, save, verify new position works

**Edge cases tested:**
- No screenshot loaded (gray background, still functional)
- Zoom in/out for precision placement
- Window resize (coordinates adjust)
- Save to non-existent directory (create dirs)

---

## Phase 8: macro Node

**Goal:** Combo sequences working.

**Deliverables:**
- `macro` node implementation:
  - Timed sequence execution
  - Stop on key release mid-sequence
  - Configurable delays between steps

**Verification:**
1. Create macro that does: tap (0.5, 0.3), wait 50ms, tap (0.55, 0.35)
2. Trigger with key press
3. Verify both touches appear in sequence

---

## Testing Strategy

### Unit Tests (per module)
- `engine.rs`: Feed synthetic events, assert correct TouchCommands
- `profile.rs`: Parse valid/invalid JSON, assert validation results
- Coordinate math: clamp, convert, normalize

### Integration Tests
- Full pipeline: synthetic evdev event → engine → uinput → verify with `evtest`
- IPC: connect to daemon, send commands, verify responses

### Manual Tests (require hardware)
- Real keyboard + mouse → real Waydroid → real game
- Each node type verified in-game
- Stress test: all nodes active simultaneously
- Duration test: 1 hour of continuous use, no leaks, no drift

---

## Dependency Lock

Pin exact versions in `Cargo.lock`. Do not use `*` or `>=` in `Cargo.toml`. Tested versions:

```toml
[dependencies]
tokio = { version = "1.38", features = ["full"] }
nix = { version = "0.27", features = ["ioctl", "fs", "signal", "poll", "socket"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
toml = "0.8"
dirs = "5.0"

# GUI only
eframe = "0.28"
egui = "0.28"
image = "0.25"
rfd = "0.14"  # file dialog
```

Verify each crate exists on crates.io with the specified version before starting development.
