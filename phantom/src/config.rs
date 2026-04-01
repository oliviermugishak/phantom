use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub screen: ScreenConfig,
    #[serde(default)]
    pub log_level: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScreenConfig {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            screen: ScreenConfig::default(),
            log_level: "info".into(),
        }
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
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
        .or_else(dirs::runtime_dir)
        .filter(|path| path.is_dir());

    if let Some(runtime) = runtime {
        runtime.join("phantom.sock")
    } else {
        PathBuf::from(format!("/tmp/phantom-{}.sock", unsafe { libc::getuid() }))
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
