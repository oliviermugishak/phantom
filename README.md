# Phantom

Virtual touchscreen mapper for Waydroid. Maps keyboard and mouse input to multitouch events via the Linux kernel's `uinput` subsystem — no ADB, no emulation detection, zero overhead.

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐     ┌─────────────┐
│  Keyboard   │     │   Phantom    │     │   uinput     │     │  Waydroid   │
│  Mouse      │────▶│   daemon     │────▶│   kernel     │────▶│  Android    │
│  evdev grab │     │   engine     │     │   virtual    │     │  sees real  │
│             │     │              │     │   touch      │     │  hardware   │
└─────────────┘     └──────────────┘     └──────────────┘     └─────────────┘
```

## Why

Waydroid runs as an LXC container on your Linux kernel. It shares the same `/dev/input` subsystem. Phantom creates a virtual multitouch device that Waydroid's Android sees as real hardware — identical to plugging in a touchscreen. No ADB spawning, no JVM overhead, no emulation detection.

## Install

### Requirements

- Linux kernel 5.1+ (for `UI_ABS_SETUP` ioctl)
- Hyprland, Sway, or any wlroots-based compositor (or X11)
- Waydroid running as LXC container
- Root access or `input` group membership

### Build

```bash
git clone <repo>
cd ttplayer
cargo build --release
```

### Setup

```bash
# Option A: Run as root
sudo ./target/release/phantom --daemon

# Option B: Set up udev rules (run as regular user)
sudo cp contrib/99-phantom.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -aG input $USER
# Log out and back in
./target/release/phantom --daemon
```

### Load a profile

```bash
# In another terminal
./target/release/phantom load profiles/pubg.json
./target/release/phantom status
```

## Usage

### Daemon

```bash
phantom --daemon              # Start the daemon (needs root or input group)
```

### CLI commands

```bash
phantom load <profile.json>   # Load a profile
phantom status                # Show daemon state
phantom pause                 # Pause input processing
phantom resume                # Resume input processing
phantom reload                # Reload current profile from disk
phantom sensitivity <value>   # Set global sensitivity (0.1 - 10.0)
phantom list                  # List available profiles
phantom shutdown              # Clean shutdown
```

### GUI

```bash
./target/release/phantom-gui   # Open profile editor
```

The GUI lets you drag node positions on a canvas, edit key bindings, adjust sensitivity, and save profiles.

## How it works

### Input capture

Phantom opens `/dev/input/event*` devices (keyboards and mice) and calls `EVIOCGRAB` to exclusively capture them. No events reach the compositor. Raw evdev events are parsed directly — key presses, releases, and mouse deltas.

### Keymap engine

A pure state machine receives raw input events and maps them to touch commands based on the loaded JSON profile. Six node types:

| Type | Use | Example |
|---|---|---|
| `tap` | Key down = touch, key up = lift | Jump, reload, crouch |
| `hold_tap` | Hold key = hold finger | Fire, aim, sprint |
| `joystick` | 4 keys map to directional movement | WASD movement |
| `mouse_camera` | Mouse delta = swipe on screen | Camera/aim control |
| `repeat_tap` | Hold key = rapid taps | Auto-fire, rapid loot |
| `macro` | One key = timed sequence | Combos, emotes |

Each node gets a fixed MT Protocol B slot (0–9). Up to 10 simultaneous touches.

### Touch injection

Phantom writes raw `input_event` structs to `/dev/uinput`. Every touch is a structured sequence:

```
EV_ABS  ABS_MT_SLOT         <slot_id>
EV_ABS  ABS_MT_TRACKING_ID  <id or -1 to lift>
EV_ABS  ABS_MT_POSITION_X   <x pixels>
EV_ABS  ABS_MT_POSITION_Y   <y pixels>
EV_SYN  SYN_REPORT          0
```

This is exactly what a real touchscreen driver does. Android cannot distinguish it from hardware.

## Profile format

Profiles are JSON files in `~/.config/phantom/profiles/`. Coordinates are relative (0.0–1.0), so profiles work at any resolution.

```json
{
  "name": "PUBG Mobile",
  "version": 1,
  "global_sensitivity": 1.0,
  "nodes": [
    {
      "id": "move",
      "type": "joystick",
      "slot": 0,
      "pos": { "x": 0.18, "y": 0.72 },
      "radius": 0.07,
      "keys": { "up": "W", "down": "S", "left": "A", "right": "D" }
    },
    {
      "id": "camera",
      "type": "mouse_camera",
      "slot": 1,
      "region": { "x": 0.35, "y": 0.0, "w": 0.65, "h": 1.0 },
      "sensitivity": 1.2
    },
    {
      "id": "fire",
      "type": "hold_tap",
      "slot": 2,
      "pos": { "x": 0.88, "y": 0.62 },
      "key": "MouseLeft"
    },
    {
      "id": "jump",
      "type": "tap",
      "slot": 3,
      "pos": { "x": 0.92, "y": 0.82 },
      "key": "Space"
    }
  ]
}
```

See [docs/PROFILES.md](docs/PROFILES.md) for the full specification of all node types.

## Configuration

`~/.config/phantom/config.toml`:

```toml
[screen]
# Override screen resolution (auto-detected if omitted)
# width = 1920
# height = 1080

# Log level: trace, debug, info, warn, error
log_level = "info"
```

## Project structure

```
phantom/
├── Cargo.toml                  # Workspace root
├── phantom/                    # Daemon crate
│   ├── src/
│   │   ├── main.rs             # Entry point, CLI, daemon loop
│   │   ├── engine.rs           # Keymap state machine (6 node types)
│   │   ├── input.rs            # evdev capture, device discovery
│   │   ├── inject.rs           # uinput virtual touch device
│   │   ├── ipc.rs              # Unix socket IPC server
│   │   ├── profile.rs          # JSON profile parsing + validation
│   │   ├── config.rs           # Config file loading
│   │   └── error.rs            # Error types
│   └── tests/
│       └── integration.rs      # Integration tests
├── phantom-gui/                # GUI crate
│   └── src/main.rs             # egui profile editor
├── profiles/                   # Example profiles
│   ├── pubg.json               # PUBG Mobile
│   └── genshin.json            # Genshin Impact
├── docs/                       # Detailed documentation
│   ├── ARCHITECTURE.md         # System design
│   ├── PROTOCOL.md             # uinput MT Protocol B details
│   ├── PROFILES.md             # Profile format specification
│   ├── IPC.md                  # Daemon communication protocol
│   ├── EDGE_CASES.md           # Edge cases and solutions
│   └── BUILD.md                # Build phases and roadmap
└── contrib/
    └── 99-phantom.rules        # udev rules for non-root access
```

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — Full system architecture, component breakdown, data flow
- [docs/PROTOCOL.md](docs/PROTOCOL.md) — Exact ioctl sequences for uinput device creation and event injection
- [docs/PROFILES.md](docs/PROFILES.md) — JSON profile format, all node types, validation rules, key names
- [docs/IPC.md](docs/IPC.md) — Unix socket protocol, all CLI commands, response format
- [docs/EDGE_CASES.md](docs/EDGE_CASES.md) — 25 edge cases with documented solutions
- [docs/BUILD.md](docs/BUILD.md) — 9-phase implementation roadmap with verification steps

## Tests

```bash
cargo test                          # Run all tests
cargo test -p phantom               # Daemon tests only
cargo test -- --ignored             # Include hardware tests (needs /dev/uinput)
```

34 tests: 15 unit (profile validation, engine state machine) + 19 integration (full scenarios, edge cases).

## License

MIT
