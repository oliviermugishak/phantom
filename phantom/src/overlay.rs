use std::env;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::config;
use crate::error::{PhantomError, Result};
use crate::mouse_touch::HostFrame;
use crate::profile::Profile;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CursorOverlayState {
    pub visible: bool,
    pub pressed: bool,
    pub screen_x: f32,
    pub screen_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OverlayFrame {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl From<HostFrame> for OverlayFrame {
    fn from(value: HostFrame) -> Self {
        Self {
            left: value.left as f32,
            top: value.top as f32,
            width: value.width as f32,
            height: value.height as f32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverlayPreviewSnapshot {
    pub profile: Profile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<OverlayFrame>,
}

pub struct OverlayPreview {
    child: Option<Child>,
    snapshot_path: PathBuf,
    log_path: PathBuf,
}

impl Default for OverlayPreview {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CursorOverlay {
    child: Option<Child>,
    state_path: PathBuf,
    log_path: PathBuf,
    last_state: Option<CursorOverlayState>,
}

impl OverlayPreview {
    pub fn new() -> Self {
        Self {
            child: None,
            snapshot_path: config::socket_path().with_file_name("phantom-overlay.json"),
            log_path: config::config_dir().join("overlay.log"),
        }
    }

    pub fn toggle(&mut self, profile: &Profile, frame: Option<OverlayFrame>) -> Result<bool> {
        self.refresh_status();
        if self.child.is_some() {
            self.stop()?;
            Ok(false)
        } else {
            self.start(profile, frame)?;
            Ok(true)
        }
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            tracing::info!("stopping experimental overlay preview");
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    pub fn is_running(&mut self) -> bool {
        self.refresh_status();
        self.child.is_some()
    }

    fn start(&mut self, profile: &Profile, frame: Option<OverlayFrame>) -> Result<()> {
        if let Some(parent) = self.snapshot_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = self.log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let snapshot = serde_json::to_string_pretty(&OverlayPreviewSnapshot {
            profile: profile.clone(),
            frame,
        })?;
        fs::write(&self.snapshot_path, snapshot)?;

        let stdout_log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        let stderr_log = stdout_log.try_clone()?;

        let gui_binary = find_gui_binary()?;
        let mut command = Command::new(&gui_binary);
        command
            .arg("--overlay")
            .arg(&self.snapshot_path)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_log))
            .stderr(Stdio::from(stderr_log));

        let invoking_uid = config::invoking_uid();
        let invoking_gid = config::invoking_gid();
        let current_uid = unsafe { libc::getuid() };
        if current_uid == 0 && invoking_uid != current_uid {
            command.uid(invoking_uid).gid(invoking_gid);
            let runtime_dir = PathBuf::from(format!("/run/user/{}", invoking_uid));
            if let Some(home) = config::invoking_home_dir() {
                command.env("HOME", &home);
                if std::env::var_os("XAUTHORITY").is_none() {
                    let xauthority = home.join(".Xauthority");
                    if xauthority.is_file() {
                        command.env("XAUTHORITY", xauthority);
                    }
                }
            }
            command.env("XDG_RUNTIME_DIR", &runtime_dir);
            if let Ok(user) = std::env::var("SUDO_USER") {
                command.env("USER", &user);
                command.env("LOGNAME", user);
            }
            propagate_display_env(&mut command, &runtime_dir);
        }

        tracing::info!(
            gui_binary = %gui_binary.display(),
            snapshot = %self.snapshot_path.display(),
            log = %self.log_path.display(),
            "launching experimental overlay preview"
        );

        let child = command.spawn().map_err(|e| {
            PhantomError::Internal(format!(
                "cannot launch overlay via {}: {}",
                gui_binary.display(),
                e
            ))
        })?;
        self.child = Some(child);
        Ok(())
    }

    fn refresh_status(&mut self) {
        let exited = if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    tracing::info!(?status, "overlay preview exited");
                    true
                }
                Ok(None) => false,
                Err(e) => {
                    tracing::warn!("overlay preview status check failed: {}", e);
                    true
                }
            }
        } else {
            false
        };
        if exited {
            self.child = None;
        }
    }
}

impl Default for CursorOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorOverlay {
    pub fn new() -> Self {
        Self {
            child: None,
            state_path: config::socket_path().with_file_name("phantom-cursor-overlay.json"),
            log_path: config::config_dir().join("cursor-overlay.log"),
            last_state: None,
        }
    }

    pub fn update(&mut self, state: CursorOverlayState) -> Result<()> {
        self.refresh_status();
        if self.child.is_none() && !state.visible {
            self.last_state = Some(state);
            return Ok(());
        }

        if self.child.is_some() && self.last_state.as_ref() == Some(&state) {
            return Ok(());
        }

        self.write_state(state)?;
        self.last_state = Some(state);

        if self.child.is_none() {
            self.start()?;
        }
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            tracing::info!("stopping menu-touch cursor overlay");
            let _ = child.kill();
            let _ = child.wait();
        }
        self.last_state = None;
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = self.log_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let stdout_log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        let stderr_log = stdout_log.try_clone()?;

        let gui_binary = find_gui_binary()?;
        let mut command = Command::new(&gui_binary);
        command
            .arg("--cursor-overlay")
            .arg(&self.state_path)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_log))
            .stderr(Stdio::from(stderr_log));

        let invoking_uid = config::invoking_uid();
        let invoking_gid = config::invoking_gid();
        let current_uid = unsafe { libc::getuid() };
        if current_uid == 0 && invoking_uid != current_uid {
            command.uid(invoking_uid).gid(invoking_gid);
            let runtime_dir = PathBuf::from(format!("/run/user/{}", invoking_uid));
            if let Some(home) = config::invoking_home_dir() {
                command.env("HOME", &home);
                if std::env::var_os("XAUTHORITY").is_none() {
                    let xauthority = home.join(".Xauthority");
                    if xauthority.is_file() {
                        command.env("XAUTHORITY", xauthority);
                    }
                }
            }
            command.env("XDG_RUNTIME_DIR", &runtime_dir);
            if let Ok(user) = std::env::var("SUDO_USER") {
                command.env("USER", &user);
                command.env("LOGNAME", user);
            }
            propagate_display_env(&mut command, &runtime_dir);
        }

        tracing::info!(
            gui_binary = %gui_binary.display(),
            state = %self.state_path.display(),
            log = %self.log_path.display(),
            "launching menu-touch cursor overlay"
        );

        let child = command.spawn().map_err(|e| {
            PhantomError::Internal(format!(
                "cannot launch cursor overlay via {}: {}",
                gui_binary.display(),
                e
            ))
        })?;
        self.child = Some(child);
        Ok(())
    }

    fn write_state(&self, state: CursorOverlayState) -> Result<()> {
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let snapshot = serde_json::to_vec(&state)?;
        fs::write(&self.state_path, snapshot)?;
        Ok(())
    }

    fn refresh_status(&mut self) {
        let exited = if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    tracing::info!(?status, "cursor overlay exited");
                    true
                }
                Ok(None) => false,
                Err(e) => {
                    tracing::warn!("cursor overlay status check failed: {}", e);
                    true
                }
            }
        } else {
            false
        };
        if exited {
            self.child = None;
        }
    }
}

fn propagate_display_env(command: &mut Command, runtime_dir: &Path) {
    copy_env_if_present(command, "DISPLAY");
    copy_env_if_present(command, "WAYLAND_DISPLAY");
    copy_env_if_present(command, "WAYLAND_SOCKET");
    copy_env_if_present(command, "XDG_SESSION_TYPE");
    copy_env_if_present(command, "DBUS_SESSION_BUS_ADDRESS");

    let has_wayland =
        env::var_os("WAYLAND_DISPLAY").is_some() || env::var_os("WAYLAND_SOCKET").is_some();
    let has_x11 = env::var_os("DISPLAY").is_some();

    if !has_wayland {
        if runtime_dir.join("wayland-0").exists() {
            command.env("WAYLAND_DISPLAY", "wayland-0");
        } else if runtime_dir.join("wayland-1").exists() {
            command.env("WAYLAND_DISPLAY", "wayland-1");
        }
    }

    if !has_x11 && PathBuf::from("/tmp/.X11-unix/X0").exists() {
        command.env("DISPLAY", ":0");
    }
}

fn copy_env_if_present(command: &mut Command, key: &str) {
    if let Some(value) = env::var_os(key) {
        command.env(key, value);
    }
}

fn find_gui_binary() -> Result<PathBuf> {
    let sibling = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("phantom-gui")))
        .filter(|path| path.is_file());
    if let Some(path) = sibling {
        return Ok(path);
    }

    if let Some(path) = find_in_path("phantom-gui") {
        return Ok(path);
    }

    Err(PhantomError::Internal(
        "cannot find phantom-gui binary for overlay preview".into(),
    ))
}

fn find_in_path(binary: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|dir| dir.join(binary))
            .find(|candidate| candidate.is_file())
    })
}
