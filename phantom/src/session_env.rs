use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config;

pub fn preferred_runtime_dir() -> PathBuf {
    let invoking = config::invoking_uid();
    let expected = PathBuf::from(format!("/run/user/{}", invoking));

    if let Some(value) = env::var_os("XDG_RUNTIME_DIR") {
        let path = PathBuf::from(value);
        if runtime_dir_belongs_to(&path, invoking) && path.is_dir() {
            return path;
        }
    }

    if expected.is_dir() {
        expected
    } else {
        PathBuf::from(format!("/run/user/{}", invoking))
    }
}

pub fn runtime_dir_belongs_to(path: &Path, invoking_uid: u32) -> bool {
    let expected = PathBuf::from(format!("/run/user/{}", invoking_uid));
    if path == expected {
        return true;
    }

    if path.starts_with("/run/user/") {
        return false;
    }

    path.is_dir()
}

pub fn infer_wayland_display(runtime_dir: &Path) -> Option<String> {
    if let Some(value) = env::var_os("WAYLAND_DISPLAY") {
        let name = value.to_string_lossy().into_owned();
        if !name.is_empty() {
            return Some(name);
        }
    }

    for index in 0..10 {
        let name = format!("wayland-{}", index);
        if runtime_dir.join(&name).exists() {
            return Some(name);
        }
    }
    None
}

pub fn infer_display() -> Option<String> {
    if let Some(value) = env::var_os("DISPLAY") {
        let name = value.to_string_lossy().into_owned();
        if !name.is_empty() {
            return Some(name);
        }
    }

    for index in 0..5 {
        if PathBuf::from(format!("/tmp/.X11-unix/X{}", index)).exists() {
            return Some(format!(":{}", index));
        }
    }
    None
}

pub fn infer_xauthority(runtime_dir: &Path) -> Option<PathBuf> {
    if let Some(value) = env::var_os("XAUTHORITY") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Some(home) = config::invoking_home_dir() {
        let home_auth = home.join(".Xauthority");
        if home_auth.is_file() {
            return Some(home_auth);
        }
    }

    let runtime_auth = runtime_dir.join("Xauthority");
    if runtime_auth.is_file() {
        return Some(runtime_auth);
    }

    if let Ok(entries) = fs::read_dir(runtime_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let looks_like_auth = name.contains("Xauthority")
                || name.contains("Xwaylandauth")
                || name.starts_with("xauth");
            if looks_like_auth && entry.path().is_file() {
                return Some(entry.path());
            }
        }
    }

    None
}

pub fn apply_invoking_identity(command: &mut Command) {
    let current_uid = unsafe { libc::getuid() };
    let invoking_uid = config::invoking_uid();
    let invoking_gid = config::invoking_gid();
    if current_uid == 0 && invoking_uid != current_uid {
        use std::os::unix::process::CommandExt;
        command.uid(invoking_uid).gid(invoking_gid);
        if let Ok(user) = env::var("SUDO_USER") {
            command.env("USER", &user);
            command.env("LOGNAME", user);
        }
    }
}

pub fn apply_desktop_env(command: &mut Command) {
    let runtime_dir = preferred_runtime_dir();
    command.env("XDG_RUNTIME_DIR", &runtime_dir);

    if let Some(home) = config::invoking_home_dir() {
        command.env("HOME", &home);
    }

    copy_env_if_present(command, "WAYLAND_SOCKET");
    copy_env_if_present(command, "XDG_SESSION_TYPE");
    copy_env_if_present(command, "DBUS_SESSION_BUS_ADDRESS");
    copy_env_if_present(command, "HYPRLAND_INSTANCE_SIGNATURE");

    if let Some(wayland) = infer_wayland_display(&runtime_dir) {
        command.env("WAYLAND_DISPLAY", wayland);
    }
    if let Some(display) = infer_display() {
        command.env("DISPLAY", display);
    }
    if let Some(xauthority) = infer_xauthority(&runtime_dir) {
        command.env("XAUTHORITY", xauthority);
    }
}

pub fn apply_session_command_env(command: &mut Command) {
    apply_invoking_identity(command);
    apply_desktop_env(command);
}

pub fn atomic_write(path: &Path, bytes: impl AsRef<[u8]>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "phantom.tmp".into());
    let tmp = path.with_file_name(format!(".{}.tmp", file_name));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn copy_env_if_present(command: &mut Command, key: &str) {
    if let Some(value) = env::var_os(key) {
        command.env(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_runtime_dir_is_rejected_for_invoking_user() {
        assert!(!runtime_dir_belongs_to(Path::new("/run/user/0"), 1000));
        assert!(runtime_dir_belongs_to(Path::new("/run/user/0"), 0));
        assert!(!runtime_dir_belongs_to(Path::new("/run/user/1001"), 1000));
    }
}
