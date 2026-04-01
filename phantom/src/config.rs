use serde::Deserialize;
use std::ffi::CStr;
use std::path::PathBuf;

use crate::input::Key;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub screen: ScreenConfig,
    #[serde(default)]
    pub log_level: String,
    #[serde(default)]
    pub touch_backend: TouchBackendKind,
    #[serde(default)]
    pub runtime_hotkeys: RuntimeHotkeysConfig,
    #[serde(default)]
    pub android: AndroidConfig,
    #[serde(default)]
    pub waydroid: WaydroidConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScreenConfig {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeHotkeysConfig {
    #[serde(default = "default_mouse_toggle_hotkey")]
    pub mouse_toggle: String,
    #[serde(default = "default_capture_toggle_hotkey")]
    pub capture_toggle: String,
    #[serde(default = "default_pause_toggle_hotkey")]
    pub pause_toggle: String,
    #[serde(default = "default_shutdown_hotkey")]
    pub shutdown: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeHotkeys {
    pub mouse_toggle: Option<Key>,
    pub capture_toggle: Option<Key>,
    pub pause_toggle: Option<Key>,
    pub shutdown: Option<Key>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TouchBackendKind {
    #[default]
    Uinput,
    AndroidSocket,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AndroidConfig {
    #[serde(default)]
    pub server_jar: Option<PathBuf>,
    #[serde(default)]
    pub auto_launch: bool,
    #[serde(default = "default_android_server_class")]
    pub server_class: String,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub container_bind_host: Option<String>,
    #[serde(default)]
    pub container_log_path: Option<String>,
    #[serde(default)]
    pub socket_path: Option<PathBuf>,
    #[serde(default)]
    pub container_socket_path: Option<String>,
    #[serde(default)]
    pub container_server_jar: Option<String>,
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
            touch_backend: TouchBackendKind::default(),
            runtime_hotkeys: RuntimeHotkeysConfig::default(),
            android: AndroidConfig::default(),
            waydroid: WaydroidConfig::default(),
        }
    }
}

impl Default for AndroidConfig {
    fn default() -> Self {
        Self {
            server_jar: None,
            auto_launch: false,
            server_class: default_android_server_class(),
            host: None,
            port: None,
            container_bind_host: None,
            container_log_path: None,
            socket_path: None,
            container_socket_path: None,
            container_server_jar: None,
        }
    }
}

impl Default for RuntimeHotkeysConfig {
    fn default() -> Self {
        Self {
            mouse_toggle: default_mouse_toggle_hotkey(),
            capture_toggle: default_capture_toggle_hotkey(),
            pause_toggle: default_pause_toggle_hotkey(),
            shutdown: default_shutdown_hotkey(),
        }
    }
}

impl Default for RuntimeHotkeys {
    fn default() -> Self {
        Self {
            mouse_toggle: Some(Key::F1),
            capture_toggle: Some(Key::F8),
            pause_toggle: Some(Key::F9),
            shutdown: Some(Key::F2),
        }
    }
}

fn default_android_server_class() -> String {
    "com.phantom.server.PhantomServer".into()
}

fn default_mouse_toggle_hotkey() -> String {
    "F1".into()
}

fn default_capture_toggle_hotkey() -> String {
    "F8".into()
}

fn default_pause_toggle_hotkey() -> String {
    "F9".into()
}

fn default_shutdown_hotkey() -> String {
    "F2".into()
}

pub fn config_dir() -> PathBuf {
    invoking_config_base_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("phantom")
}

pub fn profiles_dir() -> PathBuf {
    config_dir().join("profiles")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn load_config() -> Config {
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

pub fn resolved_runtime_hotkeys(config: &Config) -> RuntimeHotkeys {
    let defaults = RuntimeHotkeys::default();
    let resolved = RuntimeHotkeys {
        mouse_toggle: parse_hotkey(
            "runtime_hotkeys.mouse_toggle",
            &config.runtime_hotkeys.mouse_toggle,
            defaults.mouse_toggle,
        ),
        capture_toggle: parse_hotkey(
            "runtime_hotkeys.capture_toggle",
            &config.runtime_hotkeys.capture_toggle,
            defaults.capture_toggle,
        ),
        pause_toggle: parse_hotkey(
            "runtime_hotkeys.pause_toggle",
            &config.runtime_hotkeys.pause_toggle,
            defaults.pause_toggle,
        ),
        shutdown: parse_hotkey(
            "runtime_hotkeys.shutdown",
            &config.runtime_hotkeys.shutdown,
            defaults.shutdown,
        ),
    };

    if has_duplicate_hotkeys(&resolved) {
        tracing::warn!("runtime hotkeys contain duplicates; falling back to defaults F1/F8/F9/F2");
        return defaults;
    }

    resolved
}

pub fn socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .or_else(invoking_runtime_dir)
        .or_else(dirs::runtime_dir)
        .filter(|path| path.is_dir());

    if let Some(runtime) = runtime {
        runtime.join("phantom.sock")
    } else {
        PathBuf::from(format!("/tmp/phantom-{}.sock", invoking_uid()))
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

pub fn invoking_uid() -> u32 {
    std::env::var("SUDO_UID")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_else(|| unsafe { libc::getuid() })
}

pub fn invoking_gid() -> u32 {
    std::env::var("SUDO_GID")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_else(|| unsafe { libc::getgid() })
}

fn invoking_config_base_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| invoking_home_dir().map(|home| home.join(".config")))
}

fn invoking_runtime_dir() -> Option<PathBuf> {
    let path = PathBuf::from(format!("/run/user/{}", invoking_uid()));
    path.is_dir().then_some(path)
}

fn invoking_home_dir() -> Option<PathBuf> {
    home_dir_for_uid(invoking_uid()).or_else(dirs::home_dir)
}

fn home_dir_for_uid(uid: u32) -> Option<PathBuf> {
    let mut pwd = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut result = std::ptr::null_mut();
    let mut buf = vec![0u8; passwd_buf_len()];

    let rc = unsafe {
        libc::getpwuid_r(
            uid,
            pwd.as_mut_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
            &mut result,
        )
    };

    if rc != 0 || result.is_null() {
        return None;
    }

    let pwd = unsafe { pwd.assume_init() };
    if pwd.pw_dir.is_null() {
        return None;
    }

    let path = unsafe { CStr::from_ptr(pwd.pw_dir) }
        .to_str()
        .ok()
        .map(PathBuf::from)?;
    Some(path)
}

fn passwd_buf_len() -> usize {
    let suggested = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    if suggested > 0 {
        suggested as usize
    } else {
        16 * 1024
    }
}

fn parse_hotkey(field: &str, raw: &str, default: Option<Key>) -> Option<Key> {
    let value = raw.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        return None;
    }

    match value.parse::<Key>() {
        Ok(key) => Some(key),
        Err(_) => {
            tracing::warn!(
                "invalid {} '{}', falling back to {}",
                field,
                raw,
                default
                    .map(|key| format!("{:?}", key))
                    .unwrap_or_else(|| "disabled".into())
            );
            default
        }
    }
}

fn has_duplicate_hotkeys(hotkeys: &RuntimeHotkeys) -> bool {
    let mut seen = std::collections::HashSet::new();
    for key in [
        hotkeys.mouse_toggle,
        hotkeys.capture_toggle,
        hotkeys.pause_toggle,
        hotkeys.shutdown,
    ]
    .into_iter()
    .flatten()
    {
        if !seen.insert(key) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_hotkeys_default_to_current_bindings() {
        let resolved = resolved_runtime_hotkeys(&Config::default());
        assert_eq!(resolved.mouse_toggle, Some(Key::F1));
        assert_eq!(resolved.capture_toggle, Some(Key::F8));
        assert_eq!(resolved.pause_toggle, Some(Key::F9));
        assert_eq!(resolved.shutdown, Some(Key::F2));
    }

    #[test]
    fn runtime_hotkeys_accept_none_and_custom_keys() {
        let mut config = Config::default();
        config.runtime_hotkeys.mouse_toggle = "none".into();
        config.runtime_hotkeys.capture_toggle = "F7".into();

        let resolved = resolved_runtime_hotkeys(&config);
        assert_eq!(resolved.mouse_toggle, None);
        assert_eq!(resolved.capture_toggle, Some(Key::F7));
    }

    #[test]
    fn runtime_hotkeys_reject_duplicates_by_falling_back() {
        let mut config = Config::default();
        config.runtime_hotkeys.mouse_toggle = "F8".into();
        config.runtime_hotkeys.capture_toggle = "F8".into();

        let resolved = resolved_runtime_hotkeys(&config);
        assert_eq!(resolved, RuntimeHotkeys::default());
    }
}
