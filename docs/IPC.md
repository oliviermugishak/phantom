# IPC

Phantom exposes a Unix domain socket for local daemon control.

Clients:

- CLI
- GUI

Transport:

- newline-delimited JSON

## 1. Purpose

IPC exists so the daemon can stay long-lived while:

- profiles change
- capture changes
- runtime toggles change
- the GUI remains separate from the daemon

## 2. Request Model

Important requests:

- `load_profile`
- `load_profile_data`
- `reload`
- `status`
- `pause`
- `resume`
- `enter_capture`
- `exit_capture`
- `toggle_capture`
- `grab_mouse`
- `release_mouse`
- `toggle_mouse`
- `set_sensitivity`
- `list_profiles`
- `shutdown`

## 3. Example Requests

### `status`

```json
{"cmd":"status"}
```

### `load_profile`

```json
{"cmd":"load_profile","path":"~/.config/phantom/profiles/pubg.json"}
```

### `enter_capture`

```json
{"cmd":"enter_capture"}
```

### `toggle_mouse`

```json
{"cmd":"toggle_mouse"}
```

### `pause`

```json
{"cmd":"pause"}
```

### `set_sensitivity`

```json
{"cmd":"set_sensitivity","value":1.25}
```

## 4. Response Model

Responses can include:

- `ok`
- `message`
- `error`
- `profile`
- `nodes`
- `slots`
- `paused`
- `capture_active`
- `mouse_grabbed`
- `keyboard_grabbed`
- `sensitivity`
- `screen_width`
- `screen_height`
- `active_layers`

The CLI and GUI do not need identical formatting, but they do rely on the same response fields.

## 5. Runtime Semantics

### `enter_capture`

Effect:

- capture becomes active
- input devices are grabbed for gameplay

### `exit_capture`

Effect:

- active touches are released
- capture is disabled
- device grabs are released

### `grab_mouse`

Effect:

- mouse-originated gameplay events are forwarded into the engine

### `release_mouse`

Effect:

- active mouse-driven touches are released
- future mouse-originated gameplay events are suppressed
- capture may remain active

### `pause`

Effect:

- active touches are released
- engine stops producing new touch output

### `resume`

Effect:

- engine resumes normal processing

## 6. CLI Mapping

```bash
phantom --daemon
phantom load <profile.json>
phantom reload
phantom status
phantom pause
phantom resume
phantom enter-capture
phantom exit-capture
phantom toggle-capture
phantom grab-mouse
phantom release-mouse
phantom toggle-mouse
phantom sensitivity <value>
phantom list
phantom shutdown
```

## 7. Runtime Hotkeys

Daemon hotkeys are configured separately in:

- `config.toml`
- `[runtime_hotkeys]`

Defaults:

- `F1` -> toggle mouse routing
- `F8` -> toggle capture
- `F9` -> toggle pause
- `F10` -> toggle experimental debug control preview
- `F2` -> shutdown

These are handled in the daemon itself, not through the IPC socket.

## 8. Behavior Notes

- stale sockets are removed on daemon startup if they are no longer connectable
- `~` is expanded for profile paths
- runtime state changes that invalidate active touches release them before switching modes
- disabling mouse routing releases active mouse-driven touches immediately
