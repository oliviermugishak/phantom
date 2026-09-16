use std::env;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde::Deserialize;

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
    #[serde(default)]
    class: String,
    #[serde(default)]
    title: String,
    #[serde(default, rename = "initialClass")]
    initial_class: String,
}

impl HyprlandCursorClient {
    pub fn spawn() -> Result<Self> {
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

        crate::session_env::apply_session_command_env(&mut command);
        hyprland_ipc::propagate_command_env(&mut command);

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

    pub(crate) fn query_seed(&mut self) -> Result<Option<CursorSeed>> {
        self.stdin.write_all(b"cursor\n").map_err(|err| {
            PhantomError::Internal(format!("hyprland cursor helper write failed: {}", err))
        })?;
        self.stdin.flush().map_err(|err| {
            PhantomError::Internal(format!("hyprland cursor helper flush failed: {}", err))
        })?;

        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).map_err(|err| {
            PhantomError::Internal(format!("hyprland cursor helper read failed: {}", err))
        })?;
        if read == 0 {
            return Err(PhantomError::Internal(
                "hyprland cursor helper closed stdout".into(),
            ));
        }

        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("pos") => {
                let x = parse_seed_part(parts.next(), "x")?;
                let y = parse_seed_part(parts.next(), "y")?;
                let left = parse_seed_part(parts.next(), "left")?;
                let top = parse_seed_part(parts.next(), "top")?;
                let width = parse_seed_part(parts.next(), "width")?;
                let height = parse_seed_part(parts.next(), "height")?;
                Ok(Some(CursorSeed {
                    x,
                    y,
                    frame: HostFrame {
                        left,
                        top,
                        width,
                        height,
                    },
                }))
            }
            Some("none") => Ok(None),
            other => {
                tracing::warn!(reply = ?other, "hyprland cursor helper returned an unexpected reply");
                Ok(None)
            }
        }
    }

    pub(crate) fn query_frame(&mut self) -> Result<Option<HostFrame>> {
        self.stdin.write_all(b"frame\n").map_err(|err| {
            PhantomError::Internal(format!("hyprland frame helper write failed: {}", err))
        })?;
        self.stdin.flush().map_err(|err| {
            PhantomError::Internal(format!("hyprland frame helper flush failed: {}", err))
        })?;

        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).map_err(|err| {
            PhantomError::Internal(format!("hyprland frame helper read failed: {}", err))
        })?;
        if read == 0 {
            return Err(PhantomError::Internal(
                "hyprland cursor helper closed stdout".into(),
            ));
        }

        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("frame") => {
                let left = parse_seed_part(parts.next(), "left")?;
                let top = parse_seed_part(parts.next(), "top")?;
                let width = parse_seed_part(parts.next(), "width")?;
                let height = parse_seed_part(parts.next(), "height")?;
                Ok(Some(HostFrame {
                    left,
                    top,
                    width,
                    height,
                }))
            }
            Some("none") => Ok(None),
            other => {
                tracing::warn!(reply = ?other, "hyprland frame helper returned an unexpected reply");
                Ok(None)
            }
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
        match line.trim() {
            "cursor" => match query_normalized_position() {
                Ok(Some(seed)) => println!(
                    "pos {} {} {} {} {} {}",
                    seed.x,
                    seed.y,
                    seed.frame.left,
                    seed.frame.top,
                    seed.frame.width,
                    seed.frame.height
                ),
                Ok(None) => println!("none"),
                Err(err) => {
                    eprintln!("hyprland cursor query failed: {}", err);
                    println!("none");
                }
            },
            "frame" => match query_waydroid_frame() {
                Ok(Some(frame)) => println!(
                    "frame {} {} {} {}",
                    frame.left, frame.top, frame.width, frame.height
                ),
                Ok(None) => println!("none"),
                Err(err) => {
                    eprintln!("hyprland frame query failed: {}", err);
                    println!("none");
                }
            },
            _ => continue,
        }
        std::io::stdout().flush().ok();
    }
    Ok(())
}

fn query_waydroid_frame() -> Result<Option<HostFrame>> {
    let clients = match hyprland_ipc::json::<Vec<HyprClient>>("clients") {
        Ok(clients) => clients,
        Err(err) => {
            eprintln!("hyprland clients failed: {}", err);
            return Ok(None);
        }
    };
    let best = clients
        .iter()
        .filter(|client| client.mapped.unwrap_or(true))
        .filter(|client| !client.hidden.unwrap_or(false))
        .filter(|client| client_looks_like_waydroid(client))
        .filter(|client| client.size[0] > 0 && client.size[1] > 0)
        .max_by_key(|client| client.size[0].max(1) * client.size[1].max(1))
        .map(|client| HostFrame {
            left: client.at[0] as f64,
            top: client.at[1] as f64,
            width: client.size[0] as f64,
            height: client.size[1] as f64,
        });
    Ok(best)
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

fn parse_seed_part(value: Option<&str>, field: &str) -> Result<f64> {
    value
        .ok_or_else(|| PhantomError::Internal(format!("hyprland seed missing {}", field)))?
        .parse::<f64>()
        .map_err(|_| PhantomError::Internal(format!("hyprland seed has invalid {}", field)))
}

fn query_normalized_position() -> Result<Option<CursorSeed>> {
    let cursor = match hyprland_ipc::json::<HyprCursorPos>("cursorpos") {
        Ok(cursor) => cursor,
        Err(err) => {
            eprintln!("hyprland cursorpos failed: {}", err);
            return Ok(None);
        }
    };
    if let Ok(active) = hyprland_ipc::json::<HyprClient>("activewindow") {
        if client_looks_like_waydroid(&active) {
            if let Some(position) = normalize_within_client(&cursor, &active) {
                return Ok(Some(position));
            }
        }
    }

    let clients = match hyprland_ipc::json::<Vec<HyprClient>>("clients") {
        Ok(clients) => clients,
        Err(err) => {
            eprintln!("hyprland clients failed: {}", err);
            return Ok(None);
        }
    };
    let best = clients
        .iter()
        .filter(|client| client.mapped.unwrap_or(true))
        .filter(|client| !client.hidden.unwrap_or(false))
        .filter_map(|client| {
            normalize_within_client(&cursor, client).map(|position| {
                (
                    client_looks_like_waydroid(client),
                    (client.size[0].max(1) * client.size[1].max(1)) as i128,
                    position,
                )
            })
        })
        .max_by_key(|(waydroid, area, _)| (*waydroid, *area))
        .map(|(_, _, position)| position);
    Ok(best)
}

fn client_looks_like_waydroid(client: &HyprClient) -> bool {
    let haystack =
        format!("{} {} {}", client.class, client.title, client.initial_class).to_ascii_lowercase();
    haystack.contains("waydroid") || haystack.contains("android")
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
