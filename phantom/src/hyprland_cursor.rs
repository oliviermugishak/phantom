use std::env;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde::Deserialize;

use crate::config;
use crate::error::{PhantomError, Result};
use crate::hyprland_ipc;
use crate::mouse_touch::{CursorSeed, HostFrame};

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
        hyprland_ipc::propagate_command_env(&mut command, &runtime_dir);

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

    pub(crate) fn query_seed(&mut self) -> Option<CursorSeed> {
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
                let left = parts.next()?.parse::<f64>().ok()?;
                let top = parts.next()?.parse::<f64>().ok()?;
                let width = parts.next()?.parse::<f64>().ok()?;
                let height = parts.next()?.parse::<f64>().ok()?;
                Some(CursorSeed {
                    x,
                    y,
                    frame: HostFrame {
                        left,
                        top,
                        width,
                        height,
                    },
                })
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

        if let Some(seed) = query_normalized_position()? {
            println!(
                "pos {} {} {} {} {} {}",
                seed.x,
                seed.y,
                seed.frame.left,
                seed.frame.top,
                seed.frame.width,
                seed.frame.height
            );
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

fn query_normalized_position() -> Result<Option<CursorSeed>> {
    let cursor = hyprland_ipc::json::<HyprCursorPos>("cursorpos")?;
    if let Ok(active) = hyprland_ipc::json::<HyprClient>("activewindow") {
        if let Some(position) = normalize_within_client(&cursor, &active) {
            return Ok(Some(position));
        }
    }

    let clients = hyprland_ipc::json::<Vec<HyprClient>>("clients")?;
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

fn normalize_within_client(cursor: &HyprCursorPos, client: &HyprClient) -> Option<CursorSeed> {
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
    Some(CursorSeed {
        x: (local_x / width).clamp(0.0, 1.0),
        y: (local_y / height).clamp(0.0, 1.0),
        frame: HostFrame {
            left: x,
            top: y,
            width,
            height,
        },
    })
}
