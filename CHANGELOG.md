# Changelog

## 0.1.0 — Initial release

### Daemon (`phantom`)

- evdev input capture with `EVIOCGRAB` exclusive grab
- uinput virtual touchscreen via raw ioctls (MT Protocol B)
- 6 node types: tap, hold_tap, joystick, mouse_camera, repeat_tap, macro
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
