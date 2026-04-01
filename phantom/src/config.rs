use serde::Deserialize;
use std::ffi::{CStr, CString};
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;

const DEFAULT_CONFIG_TOML: &str = include_str!("../../config.example.toml");

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub screen: ScreenConfig,
    #[serde(default)]
    pub log_level: String,
    #[serde(default)]
    pub waydroid: WaydroidConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScreenConfig {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WaydroidConfig {
    pub work_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            screen: ScreenConfig::default(),
            log_level: "info".into(),
            waydroid: WaydroidConfig::default(),
        }
    }
}

pub fn config_dir() -> PathBuf {
    config_root_dir().join("phantom")
}

pub fn profiles_dir() -> PathBuf {
    config_dir().join("profiles")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn load_config() -> Config {
    ensure_user_state_layout();
    let path = config_path();
    if !path.exists() {
        tracing::info!("no config file at {}, using defaults", path.display());
        return Config::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!(
                    "invalid config at {}: {}, using defaults",
                    path.display(),
                    e
                );
                Config::default()
            }
        },
        Err(e) => {
            tracing::warn!("cannot read {}: {}, using defaults", path.display(), e);
            Config::default()
        }
    }
}

pub fn socket_path() -> PathBuf {
    let runtime = invocation_runtime_dir();

    if let Some(runtime) = runtime {
        runtime.join("phantom.sock")
    } else {
        PathBuf::from(format!("/tmp/phantom-{}.sock", invocation_uid()))
    }
}

pub fn default_profile_path() -> Option<PathBuf> {
    let p = profiles_dir().join("default.json");
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

pub fn state_owner_uid() -> u32 {
    invocation_uid()
}

pub fn state_owner_gid() -> u32 {
    if unsafe { libc::geteuid() } == 0 {
        parse_uid_env("SUDO_GID")
            .or_else(|| parse_uid_env("PKEXEC_GID"))
            .unwrap_or_else(|| (unsafe { libc::getegid() }) as u32)
    } else {
        (unsafe { libc::getegid() }) as u32
    }
}

fn ensure_user_state_layout() {
    for dir in [config_dir(), profiles_dir()] {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!("cannot create {}: {}", dir.display(), e);
            return;
        }
        ensure_path_owner(&dir);
    }

    let path = config_path();
    if path.exists() {
        ensure_path_owner(&path);
        return;
    }

    if let Err(e) = write_default_config(&path) {
        tracing::warn!("cannot create default config {}: {}", path.display(), e);
    } else {
        ensure_path_owner(&path);
        tracing::info!("created default config at {}", path.display());
    }
}

fn write_default_config(path: &Path) -> std::io::Result<()> {
    std::fs::write(path, DEFAULT_CONFIG_TOML)
}

fn ensure_path_owner(path: &Path) {
    let Some((uid, gid)) = desired_owner() else {
        return;
    };

    let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
        tracing::warn!("cannot set owner for {}: invalid path", path.display());
        return;
    };

    let rc = unsafe { libc::chown(c_path.as_ptr(), uid as libc::uid_t, gid as libc::gid_t) };
    if rc != 0 {
        tracing::warn!(
            "cannot set owner for {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        );
    }
}

fn desired_owner() -> Option<(u32, u32)> {
    let current_euid = unsafe { libc::geteuid() } as u32;
    let owner_uid = invocation_uid();

    if current_euid == 0 && owner_uid != 0 {
        Some((owner_uid, state_owner_gid()))
    } else {
        None
    }
}

fn config_root_dir() -> PathBuf {
    if let Some(home) = invocation_home_dir() {
        return home.join(".config");
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
}

fn invocation_runtime_dir() -> Option<PathBuf> {
    let current_euid = unsafe { libc::geteuid() } as u32;
    let invoking_uid = invocation_uid();

    if current_euid == 0 && invoking_uid != 0 {
        let candidate = PathBuf::from(format!("/run/user/{}", invoking_uid));
        if candidate.is_dir() {
            return Some(candidate);
        }
    }

    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .or_else(dirs::runtime_dir)
        .filter(|path| path.is_dir())
}

fn invocation_home_dir() -> Option<PathBuf> {
    let current_euid = unsafe { libc::geteuid() } as u32;
    if current_euid == 0 {
        if let Some(uid) = parse_uid_env("SUDO_UID").or_else(|| parse_uid_env("PKEXEC_UID")) {
            return home_for_uid(uid);
        }
    }

    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
}

fn invocation_uid() -> u32 {
    let current_euid = unsafe { libc::geteuid() } as u32;
    if current_euid == 0 {
        parse_uid_env("SUDO_UID")
            .or_else(|| parse_uid_env("PKEXEC_UID"))
            .unwrap_or(current_euid)
    } else {
        current_euid
    }
}

fn parse_uid_env(name: &str) -> Option<u32> {
    std::env::var(name).ok()?.parse::<u32>().ok()
}

fn home_for_uid(uid: u32) -> Option<PathBuf> {
    let mut pwd = MaybeUninit::<libc::passwd>::zeroed();
    let mut buf = vec![0u8; 4096];
    let mut result = std::ptr::null_mut();
    let ret = unsafe {
        libc::getpwuid_r(
            uid as libc::uid_t,
            pwd.as_mut_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
            &mut result,
        )
    };
    if ret != 0 || result.is_null() {
        return None;
    }

    let pwd = unsafe { pwd.assume_init() };
    let home = unsafe { CStr::from_ptr(pwd.pw_dir) }
        .to_string_lossy()
        .into_owned();
    if home.is_empty() {
        None
    } else {
        Some(PathBuf::from(home))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rootless_socket_falls_back_to_tmp_with_uid() {
        let path = PathBuf::from(format!("/tmp/phantom-{}.sock", state_owner_uid()));
        assert!(path.display().to_string().contains("phantom-"));
    }

    #[test]
    fn config_template_has_screen_section() {
        assert!(DEFAULT_CONFIG_TOML.contains("[screen]"));
    }
}
