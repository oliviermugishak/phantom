use std::env;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use crate::config;
use crate::error::{PhantomError, Result};
use crate::profile::Profile;

pub struct OverlayPreview {
    child: Option<Child>,
    snapshot_path: PathBuf,
}

impl Default for OverlayPreview {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlayPreview {
    pub fn new() -> Self {
        Self {
            child: None,
            snapshot_path: config::socket_path().with_file_name("phantom-overlay.json"),
        }
    }

    pub fn toggle(&mut self, profile: &Profile) -> Result<bool> {
        self.refresh_status();
        if self.child.is_some() {
            self.stop()?;
            Ok(false)
        } else {
            self.start(profile)?;
            Ok(true)
        }
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    pub fn is_running(&mut self) -> bool {
        self.refresh_status();
        self.child.is_some()
    }

    fn start(&mut self, profile: &Profile) -> Result<()> {
        if let Some(parent) = self.snapshot_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let snapshot = serde_json::to_string_pretty(profile)?;
        fs::write(&self.snapshot_path, snapshot)?;

        let gui_binary = find_gui_binary()?;
        let mut command = Command::new(&gui_binary);
        command
            .arg("--overlay")
            .arg(&self.snapshot_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let invoking_uid = config::invoking_uid();
        let invoking_gid = config::invoking_gid();
        let current_uid = unsafe { libc::getuid() };
        if current_uid == 0 && invoking_uid != current_uid {
            command.uid(invoking_uid).gid(invoking_gid);
            if let Some(home) = config::invoking_home_dir() {
                command.env("HOME", &home);
                if std::env::var_os("XAUTHORITY").is_none() {
                    let xauthority = home.join(".Xauthority");
                    if xauthority.is_file() {
                        command.env("XAUTHORITY", xauthority);
                    }
                }
            }
            command.env(
                "XDG_RUNTIME_DIR",
                PathBuf::from(format!("/run/user/{}", invoking_uid)),
            );
            if let Ok(user) = std::env::var("SUDO_USER") {
                command.env("USER", &user);
                command.env("LOGNAME", user);
            }
        }

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
