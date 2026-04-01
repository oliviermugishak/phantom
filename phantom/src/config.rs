use serde::Deserialize;
use std::ffi::CStr;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub screen: ScreenConfig,
    #[serde(default)]
    pub log_level: String,
    #[serde(default)]
    pub touch_backend: TouchBackendKind,
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

fn default_android_server_class() -> String {
    "com.phantom.server.PhantomServer".into()
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
