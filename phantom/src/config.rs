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
    #[serde(default)]
    pub mouse_touch: MouseTouchConfig,
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
    #[serde(default = "default_overlay_toggle_hotkey")]
    pub overlay_toggle: String,
    #[serde(default = "default_shutdown_hotkey")]
    pub shutdown: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeHotkeys {
    pub mouse_toggle: Option<Key>,
    pub capture_toggle: Option<Key>,
    pub pause_toggle: Option<Key>,
    pub overlay_toggle: Option<Key>,
    pub shutdown: Option<Key>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TouchBackendKind {
    Uinput,
    #[default]
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

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MouseTouchConfig {
    #[serde(default = "default_accel_min_gain")]
    pub accel_min_gain: f64,
    #[serde(default = "default_accel_max_gain")]
    pub accel_max_gain: f64,
    #[serde(default = "default_accel_low_speed")]
    pub accel_low_speed: f64,
    #[serde(default = "default_accel_high_speed")]
    pub accel_high_speed: f64,
    #[serde(default = "default_accel_speed_smoothing")]
    pub accel_speed_smoothing: f64,
    #[serde(default = "default_scroll_step")]
    pub scroll_step: f64,
}

impl Default for MouseTouchConfig {
    fn default() -> Self {
        Self {
            accel_min_gain: default_accel_min_gain(),
            accel_max_gain: default_accel_max_gain(),
            accel_low_speed: default_accel_low_speed(),
            accel_high_speed: default_accel_high_speed(),
            accel_speed_smoothing: default_accel_speed_smoothing(),
            scroll_step: default_scroll_step(),
        }
    }
}

fn default_accel_min_gain() -> f64 {
    1.0
}
fn default_accel_max_gain() -> f64 {
    2.6
}
fn default_accel_low_speed() -> f64 {
    0.15
}
fn default_accel_high_speed() -> f64 {
    3.0
}
fn default_accel_speed_smoothing() -> f64 {
    0.35
}
fn default_scroll_step() -> f64 {
    0.06
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
            mouse_touch: MouseTouchConfig::default(),
        }
    }
}

impl Default for AndroidConfig {
    fn default() -> Self {
        Self {
            server_jar: None,
            auto_launch: true,
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
            overlay_toggle: default_overlay_toggle_hotkey(),
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
            overlay_toggle: Some(Key::F10),
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

fn default_overlay_toggle_hotkey() -> String {
    "F10".into()
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
    let _ = ensure_user_config();
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
        overlay_toggle: parse_hotkey(
            "runtime_hotkeys.overlay_toggle",
            &config.runtime_hotkeys.overlay_toggle,
            defaults.overlay_toggle,
        ),
        shutdown: parse_hotkey(
            "runtime_hotkeys.shutdown",
            &config.runtime_hotkeys.shutdown,
            defaults.shutdown,
        ),
    };

    if has_duplicate_hotkeys(&resolved) {
        tracing::warn!(
            "runtime hotkeys contain duplicates; falling back to defaults F1/F8/F9/F10/F2"
        );
        return defaults;
    }

    resolved
}

pub fn socket_path() -> PathBuf {
    let runtime = crate::session_env::preferred_runtime_dir();
    if runtime.is_dir() {
        runtime.join("phantom.sock")
    } else {
        PathBuf::from(format!("/tmp/phantom-{}.sock", invoking_uid()))
    }
}

pub fn shipped_profile_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("../share/phantom/profiles"));
            dirs.push(parent.join("../../share/phantom/profiles"));
        }
    }
    dirs.push(PathBuf::from("/usr/share/phantom/profiles"));
    dirs.push(PathBuf::from("/usr/local/share/phantom/profiles"));
    dirs.push(PathBuf::from("profiles"));
    dirs
}

pub fn shipped_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            paths.push(parent.join("../share/phantom/config.example.toml"));
            paths.push(parent.join("../../share/phantom/config.example.toml"));
        }
    }
    paths.push(PathBuf::from("/usr/share/phantom/config.example.toml"));
    paths.push(PathBuf::from(
        "/usr/local/share/phantom/config.example.toml",
    ));
    paths.push(PathBuf::from("config.example.toml"));
    paths
}

pub fn ensure_user_config() -> bool {
    let dest = config_path();
    if dest.exists() {
        return false;
    }
    if let Some(parent) = dest.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    for source in shipped_config_paths() {
        if source.is_file() && std::fs::copy(&source, &dest).is_ok() {
            tracing::info!("created user config from {}", source.display());
            return true;
        }
    }
    false
}

pub fn seed_missing_user_profiles() -> usize {
    let dest = profiles_dir();
    if std::fs::create_dir_all(&dest).is_err() {
        return 0;
    }

    let mut seeded = 0;
    for dir in shipped_profile_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut copied_from_this_dir = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") || !path.is_file() {
                continue;
            }
            let target = dest.join(entry.file_name());
            if target.exists() {
                continue;
            }
            if std::fs::copy(&path, &target).is_ok() {
                copied_from_this_dir += 1;
                seeded += 1;
            }
        }
        if copied_from_this_dir > 0 {
            break;
        }
    }
    seeded
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

pub fn running_as_root() -> bool {
    unsafe { libc::geteuid() == 0 }
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

pub fn invoking_home_dir() -> Option<PathBuf> {
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
        hotkeys.overlay_toggle,
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
    fn missing_config_defaults_to_android_socket() {
        let cfg = Config::default();
        assert_eq!(cfg.touch_backend, TouchBackendKind::AndroidSocket);
        assert!(cfg.android.auto_launch);
    }

    #[test]
    fn runtime_hotkeys_default_to_current_bindings() {
        let resolved = resolved_runtime_hotkeys(&Config::default());
        assert_eq!(resolved.mouse_toggle, Some(Key::F1));
        assert_eq!(resolved.capture_toggle, Some(Key::F8));
        assert_eq!(resolved.pause_toggle, Some(Key::F9));
        assert_eq!(resolved.overlay_toggle, Some(Key::F10));
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
