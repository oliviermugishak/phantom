use std::io::{BufRead, BufReader as StdBufReader, Write};
use std::os::unix::net::UnixStream as StdUnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{broadcast, RwLock};

use crate::config;
use crate::desktop_relay::DesktopKeyboardRelay;
use crate::engine::{KeymapEngine, TouchCommand};
use crate::error::{PhantomError, Result};
use crate::input::{InputCapture, InputEvent};
use crate::mouse_touch::MouseTouchEmulator;
use crate::overlay::OverlayPreview;
use crate::profile::Profile;
use crate::touch::TouchDevice;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcRequest {
    LoadProfile { path: String },
    LoadProfileData { profile: Profile },
    Reload,
    Status,
    SetSensitivity { value: f64 },
    ListProfiles,
    Pause,
    Resume,
    EnterCapture,
    ExitCapture,
    ToggleCapture,
    GrabMouse,
    ReleaseMouse,
    ToggleMouse,
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
    pub capture_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mouse_grabbed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard_grabbed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mouse_touch_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mouse_touch_backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profiles: Option<Vec<ProfileEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_layers: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileEntry {
    pub name: String,
    pub path: String,
}

pub struct DaemonState {
    pub engine: RwLock<KeymapEngine>,
    pub profile_path: RwLock<Option<PathBuf>>,
    pub touch: Mutex<Box<dyn TouchDevice>>,
    pub desktop_keyboard: Mutex<DesktopKeyboardRelay>,
    pub capture: Mutex<InputCapture>,
    pub mouse_touch: Mutex<MouseTouchEmulator>,
    pub overlay: Mutex<OverlayPreview>,
    pub screen_width: u32,
    pub screen_height: u32,
    pub capture_active: AtomicBool,
    pub shutdown_tx: broadcast::Sender<()>,
}

impl DaemonState {
    pub fn new(
        engine: KeymapEngine,
        touch: Box<dyn TouchDevice>,
        desktop_keyboard: DesktopKeyboardRelay,
        capture: InputCapture,
        width: u32,
        height: u32,
    ) -> (Arc<Self>, broadcast::Receiver<()>) {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let state = Arc::new(Self {
            engine: RwLock::new(engine),
            profile_path: RwLock::new(None),
            touch: Mutex::new(touch),
            desktop_keyboard: Mutex::new(desktop_keyboard),
            capture: Mutex::new(capture),
            mouse_touch: Mutex::new(MouseTouchEmulator::new(width, height)),
            overlay: Mutex::new(OverlayPreview::new()),
            screen_width: width,
            screen_height: height,
            capture_active: AtomicBool::new(false),
            shutdown_tx,
        });
        (state, shutdown_rx)
    }
}

const IPC_IO_TIMEOUT: Duration = Duration::from_secs(5);
const IPC_MAX_LINE_BYTES: usize = 64 * 1024;

pub async fn run_ipc_server(state: Arc<DaemonState>) -> Result<()> {
    let socket_path = config::socket_path();
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| PhantomError::Ipc(format!("cannot create {}: {}", parent.display(), e)))?;
    }

    if socket_path.exists() {
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

    let listener = UnixListener::bind(&socket_path)
        .map_err(|e| PhantomError::Ipc(format!("cannot bind {}: {}", socket_path.display(), e)))?;

    let _ = std::fs::set_permissions(
        &socket_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o660),
    );
    let _ = maybe_chown_socket(&socket_path);

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

fn maybe_chown_socket(path: &std::path::Path) -> Result<()> {
    let uid = config::invoking_uid();
    let gid = config::invoking_gid();
    let current_uid = unsafe { libc::getuid() };

    if current_uid != 0 {
        return Ok(());
    }

    let path_bytes = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
        .map_err(|_| PhantomError::Ipc(format!("invalid socket path {}", path.display())))?;

    let rc = unsafe { libc::chown(path_bytes.as_ptr(), uid, gid) };
    if rc != 0 {
        return Err(PhantomError::Ipc(format!(
            "cannot chown {} to {}:{}: {}",
            path.display(),
            uid,
            gid,
            std::io::Error::last_os_error()
        )));
    }

    Ok(())
}

async fn handle_connection(stream: tokio::net::UnixStream, state: Arc<DaemonState>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    let Some(line) = read_line_limited(&mut reader, "request").await? else {
        return Ok(());
    };

    let request: IpcRequest = serde_json::from_str(line.trim())
        .map_err(|e| PhantomError::Ipc(format!("invalid JSON: {}", e)))?;

    let response = handle_request(request, &state).await;

    let json = serde_json::to_string(&response)
        .map_err(|e| PhantomError::Ipc(format!("serialize error: {}", e)))?;
    write_line_with_timeout(&mut writer, json.as_bytes(), "response").await?;
    write_line_with_timeout(&mut writer, b"\n", "response").await?;

    Ok(())
}

async fn handle_request(request: IpcRequest, state: &Arc<DaemonState>) -> IpcResponse {
    match request {
        IpcRequest::LoadProfile { path } => {
            let path = shellexpand(&path);
            match Profile::load(std::path::Path::new(&path)) {
                Ok(profile) => {
                    match load_profile_into_state(state, profile, Some(path.clone())).await {
                        Ok(response) => response,
                        Err(e) => error_response(e.to_string()),
                    }
                }
                Err(e) => error_response(e.to_string()),
            }
        }
        IpcRequest::LoadProfileData { profile } => {
            let profile = profile.normalized();
            match profile.validate() {
                Ok(()) => match load_profile_into_state(state, profile, None).await {
                    Ok(response) => response,
                    Err(e) => error_response(e.to_string()),
                },
                Err(e) => error_response(e.to_string()),
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
                None => error_response("no profile loaded".into()),
            }
        }
        IpcRequest::Status => {
            let engine = state.engine.read().await;
            let (mouse_grabbed, keyboard_grabbed) = match current_grab_state(state) {
                Ok(state) => state,
                Err(e) => return error_response(e.to_string()),
            };
            let mouse_touch_active = match mouse_touch_enabled(state) {
                Ok(active) => active,
                Err(e) => return error_response(e.to_string()),
            };
            let mouse_touch_backend = match lock_mouse_touch(state) {
                Ok(mouse_touch) => mouse_touch.backend_name().to_string(),
                Err(e) => return error_response(e.to_string()),
            };
            IpcResponse {
                ok: true,
                error: None,
                message: None,
                profile: Some(engine.profile_name().to_string()),
                profile_path: state
                    .profile_path
                    .read()
                    .await
                    .as_ref()
                    .map(|p| p.display().to_string()),
                nodes: Some(engine.node_count()),
                slots: Some(engine.slots()),
                paused: Some(engine.is_paused()),
                capture_active: Some(state.capture_active.load(Ordering::Acquire)),
                mouse_grabbed: Some(mouse_grabbed),
                keyboard_grabbed: Some(keyboard_grabbed),
                mouse_touch_active: Some(mouse_touch_active),
                mouse_touch_backend: Some(mouse_touch_backend),
                sensitivity: None,
                screen_width: Some(state.screen_width),
                screen_height: Some(state.screen_height),
                profiles: None,
                active_layers: Some(engine.active_layers().map(str::to_string).collect()),
            }
        }
        IpcRequest::SetSensitivity { value } => {
            if value <= 0.0 || value > 10.0 {
                return error_response("sensitivity must be in (0, 10]".into());
            }
            state.engine.write().await.set_sensitivity(value);
            let (mouse_grabbed, keyboard_grabbed) = match current_grab_state(state) {
                Ok(state) => state,
                Err(e) => return error_response(e.to_string()),
            };
            ok_response()
                .with_sensitivity(value)
                .with_capture_active(state.capture_active.load(Ordering::Acquire))
                .with_grab_state(mouse_grabbed, keyboard_grabbed)
        }
        IpcRequest::ListProfiles => {
            let dir = config::profiles_dir();
            let mut profiles = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "json") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(p) = serde_json::from_str::<serde_json::Value>(&content) {
                                let name = p
                                    .get("name")
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
            let mouse_touch_active = match mouse_touch_enabled(state) {
                Ok(active) => active,
                Err(e) => return error_response(e.to_string()),
            };
            let mouse_touch_backend = match lock_mouse_touch(state) {
                Ok(mouse_touch) => mouse_touch.backend_name().to_string(),
                Err(e) => return error_response(e.to_string()),
            };
            IpcResponse {
                ok: true,
                error: None,
                message: None,
                profile: None,
                profile_path: None,
                nodes: None,
                slots: None,
                paused: None,
                capture_active: Some(state.capture_active.load(Ordering::Acquire)),
                mouse_grabbed: None,
                keyboard_grabbed: None,
                mouse_touch_active: Some(mouse_touch_active),
                mouse_touch_backend: Some(mouse_touch_backend),
                sensitivity: None,
                screen_width: None,
                screen_height: None,
                profiles: Some(profiles),
                active_layers: None,
            }
        }
        IpcRequest::Pause => {
            let cmds = {
                let mut engine = state.engine.write().await;
                engine.pause()
            };
            if let Err(e) = apply_commands(state, &cmds) {
                return error_response(e.to_string());
            }
            let (mouse_grabbed, keyboard_grabbed) = match current_grab_state(state) {
                Ok(state) => state,
                Err(e) => return error_response(e.to_string()),
            };
            ok_response()
                .with_message("paused")
                .with_paused(true)
                .with_capture_active(state.capture_active.load(Ordering::Acquire))
                .with_grab_state(mouse_grabbed, keyboard_grabbed)
        }
        IpcRequest::Resume => {
            state.engine.write().await.resume();
            let (mouse_grabbed, keyboard_grabbed) = match current_grab_state(state) {
                Ok(state) => state,
                Err(e) => return error_response(e.to_string()),
            };
            ok_response()
                .with_message("resumed")
                .with_paused(false)
                .with_capture_active(state.capture_active.load(Ordering::Acquire))
                .with_grab_state(mouse_grabbed, keyboard_grabbed)
        }
        IpcRequest::EnterCapture => match set_capture_active(state, true).await {
            Ok(()) => match current_grab_state(state) {
                Ok((mouse_grabbed, keyboard_grabbed)) => ok_response()
                    .with_message("capture enabled")
                    .with_capture_active(true)
                    .with_grab_state(mouse_grabbed, keyboard_grabbed),
                Err(e) => error_response(e.to_string()),
            },
            Err(e) => error_response(e.to_string()),
        },
        IpcRequest::ExitCapture => match set_capture_active(state, false).await {
            Ok(()) => match current_grab_state(state) {
                Ok((mouse_grabbed, keyboard_grabbed)) => ok_response()
                    .with_message("capture disabled")
                    .with_capture_active(false)
                    .with_grab_state(mouse_grabbed, keyboard_grabbed),
                Err(e) => error_response(e.to_string()),
            },
            Err(e) => error_response(e.to_string()),
        },
        IpcRequest::ToggleCapture => {
            let next = !state.capture_active.load(Ordering::Acquire);
            match set_capture_active(state, next).await {
                Ok(()) => match current_grab_state(state) {
                    Ok((mouse_grabbed, keyboard_grabbed)) => ok_response()
                        .with_message(if next {
                            "capture enabled"
                        } else {
                            "capture disabled"
                        })
                        .with_capture_active(next)
                        .with_grab_state(mouse_grabbed, keyboard_grabbed),
                    Err(e) => error_response(e.to_string()),
                },
                Err(e) => error_response(e.to_string()),
            }
        }
        IpcRequest::GrabMouse => match set_mouse_routed(state, true).await {
            Ok(()) => match current_grab_state(state) {
                Ok((mouse_grabbed, keyboard_grabbed)) => ok_response()
                    .with_message("mouse routed to game")
                    .with_capture_active(state.capture_active.load(Ordering::Acquire))
                    .with_grab_state(mouse_grabbed, keyboard_grabbed),
                Err(e) => error_response(e.to_string()),
            },
            Err(e) => error_response(e.to_string()),
        },
        IpcRequest::ReleaseMouse => match set_mouse_routed(state, false).await {
            Ok(()) => match current_grab_state(state) {
                Ok((mouse_grabbed, keyboard_grabbed)) => ok_response()
                    .with_message("mouse released to desktop")
                    .with_capture_active(state.capture_active.load(Ordering::Acquire))
                    .with_grab_state(mouse_grabbed, keyboard_grabbed),
                Err(e) => error_response(e.to_string()),
            },
            Err(e) => error_response(e.to_string()),
        },
        IpcRequest::ToggleMouse => {
            let currently_grabbed = match current_grab_state(state) {
                Ok((mouse_grabbed, _)) => mouse_grabbed,
                Err(e) => return error_response(e.to_string()),
            };
            match set_mouse_routed(state, !currently_grabbed).await {
                Ok(()) => match current_grab_state(state) {
                    Ok((mouse_grabbed, keyboard_grabbed)) => ok_response()
                        .with_message(if mouse_grabbed {
                            "mouse routed to game"
                        } else {
                            "mouse released to desktop"
                        })
                        .with_capture_active(state.capture_active.load(Ordering::Acquire))
                        .with_grab_state(mouse_grabbed, keyboard_grabbed),
                    Err(e) => error_response(e.to_string()),
                },
                Err(e) => error_response(e.to_string()),
            }
        }
        IpcRequest::Shutdown => {
            let _ = state.shutdown_tx.send(());
            ok_response().with_message("shutting down")
        }
    }
}

async fn load_profile_into_state(
    state: &Arc<DaemonState>,
    profile: Profile,
    path: Option<String>,
) -> Result<IpcResponse> {
    let screen = profile
        .screen
        .as_ref()
        .ok_or_else(|| PhantomError::ProfileValidation {
            field: "screen".into(),
            message: "profile screen is required".into(),
        })?;
    if screen.width != state.screen_width || screen.height != state.screen_height {
        return Err(PhantomError::Ipc(format!(
            "profile screen {}x{} does not match daemon touchscreen {}x{}",
            screen.width, screen.height, state.screen_width, state.screen_height
        )));
    }

    let name = profile.name.clone();
    let audit = profile.audit();
    let slots: Vec<u8> = audit.touch_entries.iter().map(|entry| entry.slot).collect();
    let nodes = profile.nodes.len();

    let (was_paused, release_cmds) = {
        let mut engine = state.engine.write().await;
        let paused = engine.is_paused();
        let cmds = engine.release_all();
        (paused, cmds)
    };
    apply_commands(state, &release_cmds)?;

    let mut new_engine = KeymapEngine::new(profile);
    if was_paused {
        let _ = new_engine.pause();
    }
    *state.engine.write().await = new_engine;
    if let Some(path) = &path {
        *state.profile_path.write().await = Some(std::path::PathBuf::from(path));
    }
    let (mouse_grabbed, keyboard_grabbed) = current_grab_state(state)?;

    Ok(IpcResponse {
        ok: true,
        error: None,
        message: Some("profile loaded".into()),
        profile: Some(name),
        profile_path: path,
        nodes: Some(nodes),
        slots: Some(slots),
        paused: Some(was_paused),
        capture_active: Some(state.capture_active.load(Ordering::Acquire)),
        mouse_grabbed: Some(mouse_grabbed),
        keyboard_grabbed: Some(keyboard_grabbed),
        mouse_touch_active: Some(mouse_touch_enabled(state)?),
        mouse_touch_backend: Some(lock_mouse_touch(state)?.backend_name().to_string()),
        sensitivity: None,
        screen_width: Some(state.screen_width),
        screen_height: Some(state.screen_height),
        profiles: None,
        active_layers: Some(Vec::new()),
    })
}

pub async fn set_capture_active(state: &Arc<DaemonState>, active: bool) -> Result<()> {
    if state.capture_active.load(Ordering::Acquire) == active {
        return Ok(());
    }

    if !active {
        let cmds = {
            let mut engine = state.engine.write().await;
            engine.release_all()
        };
        apply_commands(state, &cmds)?;
    }

    let mouse_touch_cmds = {
        let mut mouse_touch = lock_mouse_touch(state)?;
        mouse_touch.suspend()
    };
    apply_commands(state, &mouse_touch_cmds)?;

    {
        let mut capture = lock_capture(state)?;
        // The daemon keeps the keyboard grabbed for its lifetime so runtime
        // hotkeys stay reliable even when gameplay capture is not active.
        capture.set_grabbed_keyboard_only(true)?;
        capture.set_grabbed_mouse_only(false)?;
    }
    state.capture_active.store(active, Ordering::Release);
    if active {
        let pressed_mouse = {
            let capture = lock_capture(state)?;
            capture.current_pressed_mouse_keys()
        };
        let cmds = {
            let mut mouse_touch = lock_mouse_touch(state)?;
            mouse_touch.resync_buttons(&pressed_mouse)
        };
        apply_commands(state, &cmds)?;

        let engine = state.engine.read().await;
        if !engine.has_mouse_camera() {
            tracing::info!(
                "capture enabled without an aim node in the loaded profile; grab the mouse only when the profile should steer the camera"
            );
        }
    }
    Ok(())
}

pub async fn set_mouse_routed(state: &Arc<DaemonState>, routed: bool) -> Result<()> {
    if !state.capture_active.load(Ordering::Acquire) {
        return Err(PhantomError::Ipc(
            "capture must be enabled before toggling mouse routing".into(),
        ));
    }

    let (current_mouse, _) = current_grab_state(state)?;
    if current_mouse == routed {
        return Ok(());
    }

    if !routed {
        let cmds = {
            let mut engine = state.engine.write().await;
            engine.suspend_mouse_inputs()
        };
        apply_commands(state, &cmds)?;
    }

    let mouse_touch_cmds = {
        let mut mouse_touch = lock_mouse_touch(state)?;
        mouse_touch.suspend()
    };
    apply_commands(state, &mouse_touch_cmds)?;

    let pressed_mouse = {
        let mut capture = lock_capture(state)?;
        capture.set_grabbed_mouse_only(routed)?;
        Some(capture.current_pressed_mouse_keys())
    };

    if let Some(pressed_mouse) = pressed_mouse {
        let cmds = {
            if routed {
                let mut engine = state.engine.write().await;
                engine.resync_mouse_buttons(&pressed_mouse)
            } else {
                let mut mouse_touch = lock_mouse_touch(state)?;
                mouse_touch.resync_buttons(&pressed_mouse)
            }
        };
        apply_commands(state, &cmds)?;
    }

    if routed {
        let engine = state.engine.read().await;
        if !engine.has_mouse_camera() {
            tracing::info!("mouse routed to gameplay, but the loaded profile has no aim node");
        }
    }
    Ok(())
}

pub fn apply_commands(state: &Arc<DaemonState>, cmds: &[TouchCommand]) -> Result<()> {
    if cmds.is_empty() {
        return Ok(());
    }

    let mut device = lock_touch_device(state)?;
    device.apply_commands(cmds)
}

fn current_grab_state(state: &Arc<DaemonState>) -> Result<(bool, bool)> {
    let capture = lock_capture(state)?;
    Ok((capture.mouse_grabbed(), capture.keyboard_grabbed()))
}

fn mouse_touch_enabled(state: &Arc<DaemonState>) -> Result<bool> {
    let (mouse_grabbed, _) = current_grab_state(state)?;
    Ok(state.capture_active.load(Ordering::Acquire) && !mouse_grabbed)
}

pub fn lock_touch_device(state: &Arc<DaemonState>) -> Result<MutexGuard<'_, Box<dyn TouchDevice>>> {
    match state.touch.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            tracing::warn!("touch backend lock poisoned, recovering");
            Ok(poisoned.into_inner())
        }
    }
}

pub fn lock_desktop_keyboard(
    state: &Arc<DaemonState>,
) -> Result<MutexGuard<'_, DesktopKeyboardRelay>> {
    match state.desktop_keyboard.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            tracing::warn!("desktop keyboard relay lock poisoned, recovering");
            Ok(poisoned.into_inner())
        }
    }
}

pub fn lock_capture(state: &Arc<DaemonState>) -> Result<MutexGuard<'_, InputCapture>> {
    match state.capture.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            tracing::warn!("input capture lock poisoned, recovering");
            Ok(poisoned.into_inner())
        }
    }
}

pub fn lock_mouse_touch(state: &Arc<DaemonState>) -> Result<MutexGuard<'_, MouseTouchEmulator>> {
    match state.mouse_touch.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            tracing::warn!("mouse-touch lock poisoned, recovering");
            Ok(poisoned.into_inner())
        }
    }
}

pub fn lock_overlay(state: &Arc<DaemonState>) -> Result<MutexGuard<'_, OverlayPreview>> {
    match state.overlay.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            tracing::warn!("overlay preview lock poisoned, recovering");
            Ok(poisoned.into_inner())
        }
    }
}

pub fn relay_keyboard_event_to_desktop(state: &Arc<DaemonState>, event: &InputEvent) -> Result<()> {
    let mut relay = lock_desktop_keyboard(state)?;
    match event {
        InputEvent::KeyPress(key) if !key.is_mouse() => relay.relay_key_event(*key, true),
        InputEvent::KeyRelease(key) if !key.is_mouse() => relay.relay_key_event(*key, false),
        _ => Ok(()),
    }
}

fn error_response(error: String) -> IpcResponse {
    IpcResponse {
        ok: false,
        error: Some(error),
        message: None,
        profile: None,
        profile_path: None,
        nodes: None,
        slots: None,
        paused: None,
        capture_active: None,
        mouse_grabbed: None,
        keyboard_grabbed: None,
        mouse_touch_active: None,
        mouse_touch_backend: None,
        sensitivity: None,
        screen_width: None,
        screen_height: None,
        profiles: None,
        active_layers: None,
    }
}

fn ok_response() -> IpcResponse {
    IpcResponse {
        ok: true,
        error: None,
        message: None,
        profile: None,
        profile_path: None,
        nodes: None,
        slots: None,
        paused: None,
        capture_active: None,
        mouse_grabbed: None,
        keyboard_grabbed: None,
        mouse_touch_active: None,
        mouse_touch_backend: None,
        sensitivity: None,
        screen_width: None,
        screen_height: None,
        profiles: None,
        active_layers: None,
    }
}

impl IpcResponse {
    fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    fn with_paused(mut self, paused: bool) -> Self {
        self.paused = Some(paused);
        self
    }

    fn with_capture_active(mut self, capture_active: bool) -> Self {
        self.capture_active = Some(capture_active);
        self
    }

    fn with_grab_state(mut self, mouse_grabbed: bool, keyboard_grabbed: bool) -> Self {
        self.mouse_grabbed = Some(mouse_grabbed);
        self.keyboard_grabbed = Some(keyboard_grabbed);
        self
    }

    fn with_sensitivity(mut self, sensitivity: f64) -> Self {
        self.sensitivity = Some(sensitivity);
        self
    }
}

fn shellexpand(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).display().to_string();
        }
    }
    s.to_string()
}

pub async fn send_command(request: &IpcRequest) -> Result<IpcResponse> {
    let socket_path = config::socket_path();
    let mut stream = tokio::net::UnixStream::connect(&socket_path)
        .await
        .map_err(|e| PhantomError::Ipc(format!("cannot connect to daemon: {}", e)))?;

    let json = serde_json::to_string(request)?;
    write_line_with_timeout(&mut stream, json.as_bytes(), "request").await?;
    write_line_with_timeout(&mut stream, b"\n", "request").await?;

    let mut reader = BufReader::new(stream);
    let line = read_line_limited(&mut reader, "response")
        .await?
        .ok_or_else(|| PhantomError::Ipc("daemon closed connection without a response".into()))?;

    serde_json::from_str(line.trim())
        .map_err(|e| PhantomError::Ipc(format!("invalid response: {}", e)))
}

pub fn send_command_blocking(request: &IpcRequest) -> Result<IpcResponse> {
    let socket_path = config::socket_path();
    let mut stream = StdUnixStream::connect(&socket_path)
        .map_err(|e| PhantomError::Ipc(format!("cannot connect to daemon: {}", e)))?;
    stream
        .set_read_timeout(Some(IPC_IO_TIMEOUT))
        .map_err(PhantomError::Io)?;
    stream
        .set_write_timeout(Some(IPC_IO_TIMEOUT))
        .map_err(PhantomError::Io)?;

    let json = serde_json::to_string(request)?;
    stream
        .write_all(json.as_bytes())
        .map_err(PhantomError::Io)?;
    stream.write_all(b"\n").map_err(PhantomError::Io)?;
    stream.flush().map_err(PhantomError::Io)?;

    let mut reader = StdBufReader::new(stream);
    let line = read_line_limited_blocking(&mut reader, "response")?
        .ok_or_else(|| PhantomError::Ipc("daemon closed connection without a response".into()))?;
    serde_json::from_str(line.trim())
        .map_err(|e| PhantomError::Ipc(format!("invalid response: {}", e)))
}

async fn read_line_limited<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    label: &str,
) -> Result<Option<String>> {
    let mut buf = Vec::new();

    loop {
        let (consumed, has_newline) = {
            let chunk = tokio::time::timeout(IPC_IO_TIMEOUT, reader.fill_buf())
                .await
                .map_err(|_| PhantomError::Ipc(format!("{} read timed out", label)))?
                .map_err(|e| PhantomError::Ipc(format!("{} read error: {}", label, e)))?;

            if chunk.is_empty() {
                if buf.is_empty() {
                    return Ok(None);
                }
                break;
            }

            let consumed = chunk
                .iter()
                .position(|&byte| byte == b'\n')
                .map_or(chunk.len(), |idx| idx + 1);
            if buf.len() + consumed > IPC_MAX_LINE_BYTES {
                return Err(PhantomError::Ipc(format!(
                    "{} exceeds {} bytes",
                    label, IPC_MAX_LINE_BYTES
                )));
            }

            buf.extend_from_slice(&chunk[..consumed]);
            (consumed, chunk[..consumed].last() == Some(&b'\n'))
        };

        reader.consume(consumed);
        if has_newline {
            break;
        }
    }

    let line = std::str::from_utf8(&buf)
        .map_err(|e| PhantomError::Ipc(format!("{} is not valid UTF-8: {}", label, e)))?;
    Ok(Some(line.trim_end_matches(&['\r', '\n'][..]).to_string()))
}

fn read_line_limited_blocking<R: BufRead>(reader: &mut R, label: &str) -> Result<Option<String>> {
    let mut buf = Vec::new();
    let consumed = reader
        .read_until(b'\n', &mut buf)
        .map_err(PhantomError::Io)?;
    if consumed == 0 {
        return Ok(None);
    }
    if buf.len() > IPC_MAX_LINE_BYTES {
        return Err(PhantomError::Ipc(format!(
            "{} exceeds {} bytes",
            label, IPC_MAX_LINE_BYTES
        )));
    }
    let line = std::str::from_utf8(&buf)
        .map_err(|e| PhantomError::Ipc(format!("{} is not valid UTF-8: {}", label, e)))?;
    Ok(Some(line.trim_end_matches(&['\r', '\n'][..]).to_string()))
}

async fn write_line_with_timeout<W: AsyncWrite + Unpin>(
    writer: &mut W,
    bytes: &[u8],
    label: &str,
) -> Result<()> {
    tokio::time::timeout(IPC_IO_TIMEOUT, writer.write_all(bytes))
        .await
        .map_err(|_| PhantomError::Ipc(format!("{} write timed out", label)))?
        .map_err(|e| PhantomError::Ipc(format!("{} write error: {}", label, e)))
}
