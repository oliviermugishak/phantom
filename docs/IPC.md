# IPC Protocol

Phantom exposes a Unix domain socket for CLI and GUI control.

## Socket Location

Path selection:

1. `$XDG_RUNTIME_DIR/phantom.sock`
2. `dirs::runtime_dir()/phantom.sock`
3. `/tmp/phantom-<uid>.sock`

Socket permissions are `0600`.

## Transport

- newline-delimited JSON
- one request per connection
- one response per connection
- 5 second read timeout
- 5 second write timeout
- 64 KiB maximum request or response line

## Response Shape

Success:

```json
{"ok": true, ...}
```

Failure:

```json
{"ok": false, "error": "human readable message"}
```

Status-style responses may also include:

- `profile`
- `profile_path`
- `paused`
- `capture_active`
- `screen_width`
- `screen_height`

## Commands

### `load_profile`

```json
{"cmd":"load_profile","path":"~/.config/phantom/profiles/pubg.json"}
```

Loads a profile from disk and installs it into the running engine.

Current behavior:

- active touches are released before the swap
- the profile `screen` must match the daemon `screen`
- the remembered `profile_path` is updated

### `load_profile_data`

```json
{
  "cmd": "load_profile_data",
  "profile": {
    "name": "PUBG Mobile",
    "version": 1,
    "screen": { "width": 1920, "height": 1080 },
    "global_sensitivity": 1.0,
    "nodes": []
  }
}
```

This is the GUI live-edit path.

Behavior:

- validates the in-memory profile
- releases active touches
- swaps the running engine
- does not need the profile to exist on disk

### `reload`

```json
{"cmd":"reload"}
```

Reloads the currently remembered on-disk profile.

### `status`

```json
{"cmd":"status"}
```

Example:

```json
{
  "ok": true,
  "profile": "PUBG Mobile",
  "profile_path": "/home/user/.config/phantom/profiles/pubg.json",
  "paused": false,
  "capture_active": true,
  "screen_width": 1920,
  "screen_height": 1080
}
```

### `pause`

```json
{"cmd":"pause"}
```

Behavior:

- releases active touches
- keeps the daemon alive
- does not automatically disable capture

### `resume`

```json
{"cmd":"resume"}
```

Resumes engine processing.

### `enter_capture`

```json
{"cmd":"enter_capture"}
```

Behavior:

- enables exclusive `EVIOCGRAB` on keyboard and mouse devices
- keeps the current profile and engine

### `exit_capture`

```json
{"cmd":"exit_capture"}
```

Behavior:

- releases active touches
- releases exclusive grabs
- lets the desktop receive input again

### `toggle_capture`

```json
{"cmd":"toggle_capture"}
```

Toggles between exclusive gameplay capture and released desktop control.

### `set_sensitivity`

```json
{"cmd":"set_sensitivity","value":1.25}
```

Allowed range is `(0, 10]`.

### `list_profiles`

```json
{"cmd":"list_profiles"}
```

Scans `~/.config/phantom/profiles` and returns profile names with paths.

### `shutdown`

```json
{"cmd":"shutdown"}
```

Gracefully stops the daemon.

## CLI Mapping

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
phantom sensitivity <value>
phantom list
phantom shutdown
```

## Runtime Hotkeys

These are handled directly by the daemon:

- `F1` toggles mouse grab while capture is already active
- `F8` toggles capture
- `F9` toggles pause

## Behavior Notes

- stale sockets are removed on daemon startup if they are not connectable
- `~` expansion is supported for `load_profile`
- `load_profile`, `load_profile_data`, `reload`, `pause`, and `exit_capture` all release active touches before changing runtime state
- the daemon can still observe hotkeys before capture is enabled because evdev supports shared readers without `EVIOCGRAB`
