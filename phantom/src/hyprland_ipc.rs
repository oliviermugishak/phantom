use std::env;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::config;
use crate::error::{PhantomError, Result};

pub(crate) fn json<T: for<'de> Deserialize<'de>>(subject: &str) -> Result<T> {
    let response = request_raw(&format!("j/{}", subject))?;
    Ok(serde_json::from_str(&response)?)
}

pub(crate) fn propagate_command_env(command: &mut Command, runtime_dir: &Path) {
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

fn request_raw(command: &str) -> Result<String> {
    let socket = socket_path()?;
    let mut stream = UnixStream::connect(&socket).map_err(|e| {
        PhantomError::Internal(format!(
            "failed to connect to Hyprland socket {}: {}",
            socket.display(),
            e
        ))
    })?;
    stream
        .write_all(command.as_bytes())
        .map_err(|e| PhantomError::Internal(format!("failed to write Hyprland command: {}", e)))?;
    stream.shutdown(std::net::Shutdown::Write).map_err(|e| {
        PhantomError::Internal(format!("failed to finalize Hyprland command: {}", e))
    })?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| PhantomError::Internal(format!("failed to read Hyprland response: {}", e)))?;
    Ok(response)
}

fn socket_path() -> Result<PathBuf> {
    let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", config::invoking_uid())));
    let signature = env::var_os("HYPRLAND_INSTANCE_SIGNATURE")
        .map(|value| value.to_string_lossy().to_string())
        .or_else(|| infer_hyprland_instance(&runtime_dir))
        .ok_or_else(|| {
            PhantomError::Internal("cannot determine Hyprland instance signature".into())
        })?;

    let socket = runtime_dir
        .join("hypr")
        .join(signature)
        .join(".socket.sock");
    if !socket.exists() {
        return Err(PhantomError::Internal(format!(
            "Hyprland control socket does not exist at {}",
            socket.display()
        )));
    }
    Ok(socket)
}

fn copy_env_if_present(command: &mut Command, key: &str) {
    if let Some(value) = env::var_os(key) {
        command.env(key, value);
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
