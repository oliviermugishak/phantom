# Changelog

## 1.0.0 — First packaged release

### Aim / Camera

- renamed and stabilized the camera primitive around `aim`
- switched real-mouse response shaping to per-axis handling so strong vertical
  recoil pull does not inflate tiny sideways noise
- widened the hidden-touch envelope for relative mouse aim to reduce
  re-centering during fast turns
- removed the old large-sweep clipping behavior from the segmented camera path
- made `while_held` re-engage recenter cleanly instead of resuming from a stale
  edge state
- added explicit `linear`, `precision`, and `balanced` aim curve presets
- kept aim routing immediate instead of adding smoothing latency

### Joystick / Movement

- reworked keyboard joystick movement around a stronger full-throw swipe model
- fixed hesitation on fast re-engage by briefly re-centering before lift on
  full release
- made same-axis overlap follow the most recently pressed direction instead of
  dropping to neutral first
- removed floating joystick mode and standardized the project on one fixed
  movement-stick model
- aligned the engine, profile schema, GUI editor, overlay preview, and docs on
  that single joystick behavior

### Packaging / Distribution

- bumped the workspace to `1.0.0`
- added GitHub Actions CI for Rust checks and Android server jar validation
- added tag-driven GitHub release publishing
- added staged packaging for:
  - release tarballs
  - Debian packages
  - Arch packages
  - AppImage
- added `SHA256SUMS` generation for published assets
- added packaged Android server jar resolution through `/usr/lib/phantom/` and
  `../lib/phantom/` relative to the running binary
- updated the systemd unit to use `/usr/bin/phantom`
- improved Android SDK build-tool detection for `d8`

### Docs

- added release packaging and distribution documentation
- updated install docs for packaged jar resolution and release assets

## 0.1.0 — Initial release

### Daemon (`phantom`)

- evdev input capture with `EVIOCGRAB` exclusive grab
- uinput virtual touchscreen via raw ioctls (MT Protocol B)
- 6 node types: tap, hold_tap, joystick, mouse_camera (now `aim`), repeat_tap, macro
- JSON profile loading with full validation (9 rules)
- Unix socket IPC with 8 commands (load, reload, status, pause, resume, sensitivity, list, shutdown)
- CLI client for all IPC commands
- Screen resolution auto-detection (sysfs, framebuffer ioctl, config fallback)
- Signal handling (SIGTERM, SIGINT) with clean shutdown
- Key repeat filtering (value==2 discarded)
- Mouse delta merging for smooth camera
- Scroll wheel mapped to key events (WheelUp/WheelDown)
- Case-insensitive key name parsing
- Config file support (config.toml)

### GUI (`phantom-gui`)

- Canvas with node position visualization
- Draggable node repositioning
- Sidebar node list with color coding by type
- Property editor for all node types
- Add/delete nodes
- Screenshot background loading
- File open/save/save-as

### Tests

- 15 unit tests (profile validation, engine state machine)
- 19 integration tests (full scenarios, edge cases, key parsing, sensitivity)
- 4 hardware tests (require /dev/uinput)

### Documentation

- Architecture overview with system diagram
- uinput MT Protocol B reference (ioctls, event sequences)
- Profile format specification (6 node types, validation rules)
- IPC protocol specification
- 25 edge cases documented with solutions
- 9-phase build roadmap
- Installation guide with udev rules and systemd service
- Contributing guide

### Example profiles

- PUBG Mobile (8 nodes: joystick, camera, fire, ADS, jump, crouch, reload, prone)
- Genshin Impact (7 nodes: joystick, camera, auto-attack, skill, burst, jump, sprint)
