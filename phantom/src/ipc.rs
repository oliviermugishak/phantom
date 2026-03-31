use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{broadcast, RwLock};

use crate::config;
use crate::error::{PhantomError, Result};
use crate::engine::KeymapEngine;
use crate::profile::Profile;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcRequest {
    LoadProfile { path: String },
    Reload,
    Status,
    SetSensitivity { value: f64 },
    ListProfiles,
    Pause,
    Resume,
    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slots: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profiles: Option<Vec<ProfileEntry>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileEntry {
    pub name: String,
    pub path: String,
}

/// Shared state between daemon and IPC handler.
pub struct DaemonState {
    pub engine: RwLock<KeymapEngine>,
    pub profile_path: RwLock<Option<PathBuf>>,
    pub paused: RwLock<bool>,
    pub screen_width: u32,
    pub screen_height: u32,
    pub shutdown_tx: broadcast::Sender<()>,
}

impl DaemonState {
    pub fn new(engine: KeymapEngine, width: u32, height: u32) -> (Arc<Self>, broadcast::Receiver<()>) {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let state = Arc::new(Self {
            engine: RwLock::new(engine),
            profile_path: RwLock::new(None),
            paused: RwLock::new(false),
            screen_width: width,
            screen_height: height,
            shutdown_tx,
        });
        (state, shutdown_rx)
    }
}

pub async fn run_ipc_server(state: Arc<DaemonState>) -> Result<()> {
    let socket_path = config::socket_path();

    // Clean up stale socket
    if socket_path.exists() {
        // Try connecting to see if another daemon is running
        match tokio::net::UnixStream::connect(&socket_path).await {
            Ok(_) => {
                return Err(PhantomError::DaemonAlreadyRunning(
                    socket_path.display().to_string(),
                ));
            }
            Err(_) => {
                tracing::info!("removing stale socket at {}", socket_path.display());
                let _ = std::fs::remove_file(&socket_path);
            }
        }
    }

    let listener = UnixListener::bind(&socket_path).map_err(|e| {
        PhantomError::Ipc(format!("cannot bind {}: {}", socket_path.display(), e))
    })?;

    // Set permissions to 0600
    let _ = std::fs::set_permissions(
        &socket_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    );

    tracing::info!("IPC server listening on {}", socket_path.display());

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("IPC accept error: {}", e);
                continue;
            }
        };

        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, state).await {
                tracing::warn!("IPC connection error: {}", e);
            }
        });
    }
}

async fn handle_connection(
    stream: tokio::net::UnixStream,
    state: Arc<DaemonState>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Read request (one line)
    let n = reader.read_line(&mut line).await.map_err(|e| {
        PhantomError::Ipc(format!("read error: {}", e))
    })?;
    if n == 0 {
        return Ok(());
    }

    // Parse request
    let request: IpcRequest = serde_json::from_str(line.trim()).map_err(|e| {
        PhantomError::Ipc(format!("invalid JSON: {}", e))
    })?;

    // Process request
    let response = handle_request(request, &state).await;

    // Write response
    let json = serde_json::to_string(&response).map_err(|e| {
        PhantomError::Ipc(format!("serialize error: {}", e))
    })?;
    writer.write_all(json.as_bytes()).await.map_err(|e| {
        PhantomError::Ipc(format!("write error: {}", e))
    })?;
    writer.write_all(b"\n").await.map_err(|e| {
        PhantomError::Ipc(format!("write error: {}", e))
    })?;

    Ok(())
}

async fn handle_request(request: IpcRequest, state: &Arc<DaemonState>) -> IpcResponse {
    match request {
        IpcRequest::LoadProfile { path } => {
            let path = shellexpand(&path);
            match Profile::load(std::path::Path::new(&path)) {
                Ok(profile) => {
                    let name = profile.name.clone();
                    let slots: Vec<u8> = profile.nodes.iter().filter_map(|n| n.slot()).collect();
                    let nodes = profile.nodes.len();
                    let new_engine = KeymapEngine::new(profile);
                    *state.engine.write().await = new_engine;
                    *state.profile_path.write().await = Some(std::path::PathBuf::from(&path));
                    tracing::info!("loaded profile: {}", name);
                    IpcResponse {
                        ok: true,
                        error: None,
                        message: Some("profile loaded".into()),
                        profile: Some(name),
                        profile_path: Some(path),
                        nodes: Some(nodes),
                        slots: Some(slots),
                        paused: None,
                        sensitivity: None,
                        profiles: None,
                    }
                }
                Err(e) => IpcResponse {
                    ok: false,
                    error: Some(e.to_string()),
                    message: None,
                    profile: None,
                    profile_path: None,
                    nodes: None,
                    slots: None,
                    paused: None,
                    sensitivity: None,
                    profiles: None,
                },
            }
        }
        IpcRequest::Reload => {
            let path = state.profile_path.read().await.clone();
            match path {
                Some(p) => {
                    let path_str = p.display().to_string();
                    let request = IpcRequest::LoadProfile { path: path_str };
                    Box::pin(handle_request(request, state)).await
                }
                None => IpcResponse {
                    ok: false,
                    error: Some("no profile loaded".into()),
                    message: None,
                    profile: None,
                    profile_path: None,
                    nodes: None,
                    slots: None,
                    paused: None,
                    sensitivity: None,
                    profiles: None,
                },
            }
        }
        IpcRequest::Status => {
            let engine = state.engine.read().await;
            let paused = *state.paused.read().await;
            IpcResponse {
                ok: true,
                error: None,
                message: None,
                profile: Some(engine.profile_name().to_string()),
                profile_path: state.profile_path.read().await.as_ref().map(|p| p.display().to_string()),
                nodes: None,
                slots: None,
                paused: Some(paused),
                sensitivity: None,
                profiles: None,
            }
        }
        IpcRequest::SetSensitivity { value } => {
            if value <= 0.0 || value > 10.0 {
                return IpcResponse {
                    ok: false,
                    error: Some("sensitivity must be in (0, 10]".into()),
                    message: None,
                    profile: None,
                    profile_path: None,
                    nodes: None,
                    slots: None,
                    paused: None,
                    sensitivity: None,
                    profiles: None,
                };
            }
            state.engine.write().await.set_sensitivity(value);
            IpcResponse {
                ok: true,
                error: None,
                message: None,
                profile: None,
                profile_path: None,
                nodes: None,
                slots: None,
                paused: None,
                sensitivity: Some(value),
                profiles: None,
            }
        }
        IpcRequest::ListProfiles => {
            let dir = config::profiles_dir();
            let mut profiles = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |e| e == "json") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(p) = serde_json::from_str::<serde_json::Value>(&content) {
                                let name = p.get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                profiles.push(ProfileEntry {
                                    name,
                                    path: path.display().to_string(),
                                });
                            }
                        }
                    }
                }
            }
            IpcResponse {
                ok: true,
                error: None,
                message: None,
                profile: None,
                profile_path: None,
                nodes: None,
                slots: None,
                paused: None,
                sensitivity: None,
                profiles: Some(profiles),
            }
        }
        IpcRequest::Pause => {
            let mut engine = state.engine.write().await;
            let _cmds = engine.pause();
            *state.paused.write().await = true;
            IpcResponse {
                ok: true,
                error: None,
                message: Some("paused".into()),
                profile: None,
                profile_path: None,
                nodes: None,
                slots: None,
                paused: Some(true),
                sensitivity: None,
                profiles: None,
            }
        }
        IpcRequest::Resume => {
            state.engine.write().await.resume();
            *state.paused.write().await = false;
            IpcResponse {
                ok: true,
                error: None,
                message: Some("resumed".into()),
                profile: None,
                profile_path: None,
                nodes: None,
                slots: None,
                paused: Some(false),
                sensitivity: None,
                profiles: None,
            }
        }
        IpcRequest::Shutdown => {
            let _ = state.shutdown_tx.send(());
            IpcResponse {
                ok: true,
                error: None,
                message: Some("shutting down".into()),
                profile: None,
                profile_path: None,
                nodes: None,
                slots: None,
                paused: None,
                sensitivity: None,
                profiles: None,
            }
        }
    }
}

fn shellexpand(s: &str) -> String {
    if s.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&s[2..]).display().to_string();
        }
    }
    s.to_string()
}

/// Send a command to the running daemon via IPC.
pub async fn send_command(request: &IpcRequest) -> Result<IpcResponse> {
    let socket_path = config::socket_path();
    let mut stream = tokio::net::UnixStream::connect(&socket_path).await.map_err(|e| {
        PhantomError::Ipc(format!("cannot connect to daemon: {}", e))
    })?;

    let json = serde_json::to_string(request)?;
    stream.write_all(json.as_bytes()).await.map_err(|e| {
        PhantomError::Ipc(format!("write error: {}", e))
    })?;
    stream.write_all(b"\n").await.map_err(|e| {
        PhantomError::Ipc(format!("write error: {}", e))
    })?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.map_err(|e| {
        PhantomError::Ipc(format!("read error: {}", e))
    })?;

    serde_json::from_str(line.trim()).map_err(|e| {
        PhantomError::Ipc(format!("invalid response: {}", e)).into()
    })
}
