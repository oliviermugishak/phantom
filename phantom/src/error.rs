use thiserror::Error;

#[derive(Error, Debug)]
pub enum PhantomError {
    #[error("permission denied on {path}: {reason}")]
    PermissionDenied { path: String, reason: String },

    #[error("device not found: {path}")]
    DeviceNotFound { path: String },

    #[error("ioctl failed: {operation} on {path}: {reason}")]
    IoctlFailed {
        operation: String,
        path: String,
        reason: String,
    },

    #[error("profile error: {0}")]
    Profile(String),

    #[error("profile validation: {field} — {message}")]
    ProfileValidation { field: String, message: String },

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("daemon already running (socket: {0})")]
    DaemonAlreadyRunning(String),

    #[error("screen resolution detection failed: {0}")]
    ResolutionDetection(String),

    #[error("no input devices found")]
    NoInputDevices,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Toml(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, PhantomError>;
