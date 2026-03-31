# IPC Protocol — Daemon Communication

Phantom daemon exposes a Unix domain socket for external control. The GUI and CLI tools use this to manage profiles and query state.

---

## Socket

**Path:** `$XDG_RUNTIME_DIR/phantom.sock` (typically `/run/user/1000/phantom.sock`)

**Permissions:** `0600` (owner read/write only)

**Protocol:** Newline-delimited JSON (NDJSON). One JSON object per line. Each connection sends exactly one request and receives exactly one response, then the connection closes.

---

## Request Format

```json
{"cmd": "<command>", ...params}
```

Every request must have a `cmd` field. Additional fields depend on the command.

## Response Format

```json
{"ok": true, ...data}
{"ok": false, "error": "<error message>"}
```

Every response has `ok: bool`. On success, additional fields provide the result. On failure, `error` contains a human-readable description.

---

## Commands

### load_profile

Load a profile from disk. Replaces the currently active profile.

**Request:**
```json
{"cmd": "load_profile", "path": "~/.config/phantom/profiles/pubg.json"}
```

**Success:**
```json
{"ok": true, "profile": "PUBG Mobile", "nodes": 8, "slots": [0,1,2,3,4,5,6,7]}
```

**Errors:**
- File not found
- JSON parse error (with line/column)
- Validation error (with specific field)

### reload

Reload the current profile from disk. Useful after editing the JSON file externally.

**Request:**
```json
{"cmd": "reload"}
```

**Success:**
```json
{"ok": true, "profile": "PUBG Mobile", "reloaded": true}
```

### status

Query daemon state.

**Request:**
```json
{"cmd": "status"}
```

**Success:**
```json
{
  "ok": true,
  "running": true,
  "profile": "PUBG Mobile",
  "profile_path": "/home/user/.config/phantom/profiles/pubg.json",
  "active_slots": [0, 1, 2],
  "devices_grabbed": ["event3", "event5"],
  "screen": { "width": 1920, "height": 1080 },
  "uptime_seconds": 3420
}
```

### pause

Temporarily stop processing input events. All active touches are lifted. Devices remain grabbed.

**Request:**
```json
{"cmd": "pause"}
```

**Success:**
```json
{"ok": true, "paused": true}
```

### resume

Resume input processing after pause.

**Request:**
```json
{"cmd": "resume"}
```

**Success:**
```json
{"ok": true, "paused": false}
```

### set_sensitivity

Override the global sensitivity multiplier without reloading the profile.

**Request:**
```json
{"cmd": "set_sensitivity", "value": 1.5}
```

**Success:**
```json
{"ok": true, "sensitivity": 1.5}
```

**Errors:**
- Value out of range (must be 0.1–10.0)

### list_profiles

List available profiles in the profiles directory.

**Request:**
```json
{"cmd": "list_profiles"}
```

**Success:**
```json
{
  "ok": true,
  "profiles": [
    {"name": "PUBG Mobile", "path": "/home/user/.config/phantom/profiles/pubg.json"},
    {"name": "Genshin Impact", "path": "/home/user/.config/phantom/profiles/genshin.json"}
  ]
}
```

### shutdown

Graceful daemon shutdown. Lifts all touches, destroys uinput device, releases grabs.

**Request:**
```json
{"cmd": "shutdown"}
```

**Success:**
```json
{"ok": true, "shutting_down": true}
```

---

## CLI Usage

The `phantom` binary acts as both daemon (with `--daemon` flag) and CLI client.

```bash
# Start daemon
sudo phantom --daemon

# Load a profile
phantom load ~/.config/phantom/profiles/pubg.json

# Check status
phantom status

# Pause/resume
phantom pause
phantom resume

# Reload current profile
phantom reload

# Shutdown daemon
phantom shutdown
```

Without `--daemon`, the binary connects to the existing daemon's socket and sends the command.

---

## Connection Handling

- Server uses `tokio::net::UnixListener`
- Each connection is handled in a separate task
- Read until newline, parse JSON, execute command, write response + newline, close
- Timeout: 5 seconds for read, 5 seconds for write. Kill connection on timeout.
- Maximum request size: 64KB (reject larger with error)
- If socket already exists on daemon start, attempt to connect. If successful, another daemon is running — exit with error. If connection fails (stale socket), delete and re-create.
