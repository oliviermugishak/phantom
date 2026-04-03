use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde::Deserialize;

use crate::config;
use crate::error::{PhantomError, Result};

const HELPER_SUBCOMMAND: &str = "__hyprland_cursor_helper";

#[derive(Debug)]
pub struct HyprlandCursorClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[derive(Debug, Deserialize)]
struct HyprCursorPos {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct HyprClient {
    #[serde(default)]
    at: [i64; 2],
    #[serde(default)]
    size: [i64; 2],
    #[serde(default)]
    mapped: Option<bool>,
    #[serde(default)]
    hidden: Option<bool>,
}

impl HyprlandCursorClient {
    pub fn spawn() -> Result<Self> {
        let current_uid = unsafe { libc::getuid() };
        let invoking_uid = config::invoking_uid();
        let invoking_gid = config::invoking_gid();

        let binary = env::current_exe().map_err(|e| {
            PhantomError::Internal(format!(
                "cannot locate phantom binary for hyprland helper: {}",
                e
            ))
        })?;

        let mut command = Command::new(&binary);
        command
            .arg(HELPER_SUBCOMMAND)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let runtime_dir = PathBuf::from(format!("/run/user/{}", invoking_uid));
        if current_uid == 0 && invoking_uid != current_uid {
            command.uid(invoking_uid).gid(invoking_gid);
            if let Some(home) = config::invoking_home_dir() {
                command.env("HOME", &home);
            }
            if let Ok(user) = env::var("SUDO_USER") {
                command.env("USER", &user);
                command.env("LOGNAME", user);
            }
            command.env("XDG_RUNTIME_DIR", &runtime_dir);
        }
        propagate_hyprland_env(&mut command, &runtime_dir);

        let mut child = command.spawn().map_err(|e| {
            PhantomError::Internal(format!(
                "cannot launch hyprland cursor helper via {}: {}",
                binary.display(),
                e
            ))
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            PhantomError::Internal("hyprland cursor helper stdin unavailable".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            PhantomError::Internal("hyprland cursor helper stdout unavailable".into())
        })?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    pub fn query_position(&mut self) -> Option<(f64, f64)> {
        if self.stdin.write_all(b"cursor\n").is_err() || self.stdin.flush().is_err() {
            return None;
        }

        let mut line = String::new();
        if self.stdout.read_line(&mut line).ok()? == 0 {
            return None;
        }

        let mut parts = line.split_whitespace();
        match parts.next()? {
            "pos" => {
                let x = parts.next()?.parse::<f64>().ok()?;
                let y = parts.next()?.parse::<f64>().ok()?;
                Some((x, y))
            }
            _ => None,
        }
    }
}

impl Drop for HyprlandCursorClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn run_helper_stdio() -> Result<()> {
    while let Some(line) = read_line()? {
        if line.trim() != "cursor" {
            continue;
        }

        if let Some((x, y)) = query_normalized_position()? {
            println!("pos {} {}", x, y);
        } else {
            println!("none");
        }
        std::io::stdout().flush().ok();
    }
    Ok(())
}

fn read_line() -> Result<Option<String>> {
    let mut line = String::new();
    let read = std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| PhantomError::Internal(format!("hyprland helper stdin read failed: {}", e)))?;
    if read == 0 {
        return Ok(None);
    }
    Ok(Some(line))
}

fn query_normalized_position() -> Result<Option<(f64, f64)>> {
    ensure_hyprland_env();
    let cursor = hyprctl_json::<HyprCursorPos>("cursorpos")?;
    if let Ok(active) = hyprctl_json::<HyprClient>("activewindow") {
        if let Some(position) = normalize_within_client(&cursor, &active) {
            return Ok(Some(position));
        }
    }

    let clients = hyprctl_json::<Vec<HyprClient>>("clients")?;
    let best = clients
        .iter()
        .filter(|client| client.mapped.unwrap_or(true))
        .filter(|client| !client.hidden.unwrap_or(false))
        .filter_map(|client| {
            normalize_within_client(&cursor, client).map(|position| {
                let area = (client.size[0].max(1) * client.size[1].max(1)) as i128;
                (area, position)
            })
        })
        .min_by_key(|(area, _)| *area)
        .map(|(_, position)| position);
    Ok(best)
}

fn normalize_within_client(cursor: &HyprCursorPos, client: &HyprClient) -> Option<(f64, f64)> {
    let x = client.at[0] as f64;
    let y = client.at[1] as f64;
    let width = client.size[0] as f64;
    let height = client.size[1] as f64;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let local_x = cursor.x - x;
    let local_y = cursor.y - y;
    if !(0.0..=width).contains(&local_x) || !(0.0..=height).contains(&local_y) {
        return None;
    }
    Some((
        (local_x / width).clamp(0.0, 1.0),
        (local_y / height).clamp(0.0, 1.0),
    ))
}

fn hyprctl_json<T: for<'de> Deserialize<'de>>(subject: &str) -> Result<T> {
    let output = Command::new("hyprctl")
        .arg("-j")
        .arg(subject)
        .output()
        .map_err(|e| PhantomError::Internal(format!("failed to run hyprctl {}: {}", subject, e)))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(PhantomError::Internal(format!(
            "hyprctl {} failed: {}{}{}",
            subject,
            output.status,
            if stderr.is_empty() { "" } else { " stderr=" },
            if stderr.is_empty() { stdout } else { stderr }
        )));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn propagate_hyprland_env(command: &mut Command, runtime_dir: &Path) {
    copy_env_if_present(command, "HYPRLAND_INSTANCE_SIGNATURE");
    copy_env_if_present(command, "XDG_RUNTIME_DIR");
    copy_env_if_present(command, "DBUS_SESSION_BUS_ADDRESS");
    copy_env_if_present(command, "WAYLAND_DISPLAY");
    copy_env_if_present(command, "XDG_SESSION_TYPE");

    if env::var_os("XDG_RUNTIME_DIR").is_none() && runtime_dir.is_dir() {
        command.env("XDG_RUNTIME_DIR", runtime_dir);
    }

    if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        if let Some(signature) = infer_hyprland_instance(runtime_dir) {
            command.env("HYPRLAND_INSTANCE_SIGNATURE", signature);
        }
    }
}

fn copy_env_if_present(command: &mut Command, key: &str) {
    if let Some(value) = env::var_os(key) {
        command.env(key, value);
    }
}

fn ensure_hyprland_env() {
    if env::var_os("XDG_RUNTIME_DIR").is_none() {
        let runtime_dir = PathBuf::from(format!("/run/user/{}", config::invoking_uid()));
        if runtime_dir.is_dir() {
            unsafe { env::set_var("XDG_RUNTIME_DIR", runtime_dir) };
        }
    }

    if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", config::invoking_uid())));
        if let Some(signature) = infer_hyprland_instance(&runtime_dir) {
            unsafe { env::set_var("HYPRLAND_INSTANCE_SIGNATURE", signature) };
        }
    }
}

fn infer_hyprland_instance(runtime_dir: &Path) -> Option<String> {
    let hypr_dir = runtime_dir.join("hypr");
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    for entry in fs::read_dir(hypr_dir).ok()? {
        let entry = entry.ok()?;
        let file_type = entry.file_type().ok()?;
        if !file_type.is_dir() {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let name = entry.file_name().to_string_lossy().to_string();
        match &newest {
            Some((best, _)) if &modified <= best => {}
            _ => newest = Some((modified, name)),
        }
    }
    newest.map(|(_, name)| name)
}
