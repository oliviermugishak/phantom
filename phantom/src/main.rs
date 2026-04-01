use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tracing_subscriber::EnvFilter;

use phantom::config;
use phantom::engine::KeymapEngine;
use phantom::error::{PhantomError, Result};
use phantom::input::{InputCapture, InputEvent, Key};
use phantom::ipc::{self, DaemonState, IpcRequest};
use phantom::profile::Profile;
use phantom::touch;

const CAPTURE_TOGGLE_KEY: Key = Key::F8;
const PAUSE_TOGGLE_KEY: Key = Key::F9;
const MOUSE_TOGGLE_KEY: Key = Key::F1;
const RELEASE_ALL_KEY: Key = Key::F2;

fn print_help() {
    eprintln!(
        r#"phantom — virtual touchscreen for Waydroid

USAGE:
    phantom --daemon                  Start the daemon
    phantom audit <profile.json>      Audit slot and binding usage in a profile
    phantom load <profile.json>       Load a profile
    phantom status                    Show daemon status
    phantom pause                     Pause input processing
    phantom resume                    Resume input processing
    phantom reload                    Reload current profile
    phantom enter-capture             Enable exclusive gameplay capture
    phantom exit-capture              Release exclusive gameplay capture
    phantom toggle-capture            Toggle gameplay capture
    phantom sensitivity <value>       Set global sensitivity
    phantom list                      List available profiles
    phantom shutdown                  Graceful shutdown

KEYS (while daemon running):
    F2   Shutdown daemon (kills everything, restart to play again)
    F1   Toggle mouse grab (free mouse for desktop)
    F8   Toggle capture mode (game mode on/off)
    F9   Toggle pause (freeze touch injection)

FLAGS:
    --daemon      Run as daemon (requires root)
    --trace       Force trace logging for this run
    -h, --help    Show this help
    -V, --version Show version"#
    );
}

fn print_version() {
    eprintln!("phantom {}", env!("CARGO_PKG_VERSION"));
}

fn expand_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).display().to_string();
        }
    }
    path.to_string()
}

fn format_slots(slots: &[u8]) -> String {
    let mut slots = slots.to_vec();
    slots.sort_unstable();
    slots.dedup();
    if slots.is_empty() {
        "(none)".into()
    } else {
        slots
            .into_iter()
            .map(|slot| slot.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn format_bindings(bindings: &[String]) -> String {
    if bindings.is_empty() {
        "(none)".into()
    } else {
        bindings.join(", ")
    }
}

fn print_profile_audit(profile: &Profile) {
    let audit = profile.audit();
    let slots: Vec<u8> = audit.touch_entries.iter().map(|entry| entry.slot).collect();

    eprintln!("profile: {}", audit.profile_name);
    eprintln!("screen: {}x{}", audit.screen_width, audit.screen_height);
    eprintln!("total nodes: {}", audit.total_nodes);
    eprintln!("touch nodes: {}", audit.touch_entries.len());
    eprintln!("touch slots: {}", format_slots(&slots));

    if !audit.touch_entries.is_empty() {
        eprintln!("touch slot audit:");
        for entry in &audit.touch_entries {
            let mut line = format!(
                "  slot {}  {}  id={} layer={} bindings={}",
                entry.slot,
                entry.node_type,
                entry.node_id,
                entry.layer,
                format_bindings(&entry.bindings)
            );
            if let Some(detail) = &entry.detail {
                line.push_str(&format!(" {}", detail));
            }
            eprintln!("{line}");
        }
    }

    if !audit.auxiliary_entries.is_empty() {
        eprintln!("auxiliary nodes:");
        for entry in &audit.auxiliary_entries {
            let mut line = format!(
                "  {}  id={} layer={} bindings={}",
                entry.node_type,
                entry.node_id,
                entry.layer,
                format_bindings(&entry.bindings)
            );
            if let Some(detail) = &entry.detail {
                line.push_str(&format!(" {}", detail));
            }
            eprintln!("{line}");
        }
    }
}

fn init_logging(force_trace: bool) {
    let default_level = if force_trace
        || matches!(
            std::env::var("PHANTOM_TRACE").ok().as_deref(),
            Some("1" | "true" | "TRUE" | "yes" | "YES")
        ) {
        "trace".to_string()
    } else {
        let cfg = config::load_config();
        if cfg.log_level.trim().is_empty() {
            "info".to_string()
        } else {
            cfg.log_level
        }
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .init();
}

fn detect_resolution(config: &config::Config, profile: Option<&Profile>) -> Result<(u32, u32)> {
    match (config.screen.width, config.screen.height) {
        (Some(w), Some(h)) => {
            tracing::info!("screen resolution from config: {}x{}", w, h);
            Ok((w, h))
        }
        (Some(_), None) | (None, Some(_)) => Err(PhantomError::ResolutionDetection(
            "config screen requires both width and height".into(),
        )),
        (None, None) => {
            if let Some(screen) = profile.and_then(|p| p.screen.as_ref()) {
                tracing::info!(
                    "screen resolution from default profile '{}': {}x{}",
                    profile.map(|p| p.name.as_str()).unwrap_or("unknown"),
                    screen.width,
                    screen.height
                );
                Ok((screen.width, screen.height))
            } else {
                Err(PhantomError::ResolutionDetection(
                    "fullscreen mode requires an explicit resolution in config.toml or the default profile".into(),
                ))
            }
        }
    }
}

async fn run_daemon() -> Result<()> {
    tracing::info!("phantom {} starting", env!("CARGO_PKG_VERSION"));

    let config = config::load_config();
    let default_profile_path = config::default_profile_path();
    let default_profile = default_profile_path.as_ref().and_then(|path| {
        tracing::info!("loading default profile: {}", path.display());
        match Profile::load(path) {
            Ok(profile) => Some(profile),
            Err(e) => {
                tracing::warn!("failed to load default profile {}: {}", path.display(), e);
                None
            }
        }
    });

    let (screen_w, screen_h) = detect_resolution(&config, default_profile.as_ref())?;
    let touch = touch::create_touch_device(&config, screen_w, screen_h)?;
    let capture = InputCapture::discover()?;
    tracing::info!(
        devices = capture.device_count(),
        mouse = capture.has_mouse(),
        keyboard = capture.has_keyboard(),
        "input capture ready"
    );

    let engine = match default_profile {
        Some(p) => KeymapEngine::new(p),
        None => {
            tracing::warn!("no default profile found, engine idle until profile loaded via IPC");
            KeymapEngine::new(phantom::profile::Profile {
                name: "empty".into(),
                version: 1,
                screen: Some(phantom::profile::ScreenOverride {
                    width: screen_w,
                    height: screen_h,
                }),
                global_sensitivity: 1.0,
                nodes: vec![],
            })
        }
    };

    let (state, mut shutdown_rx) = DaemonState::new(engine, touch, capture, screen_w, screen_h);
    if let Some(path) = default_profile_path {
        *state.profile_path.write().await = Some(path);
    }
    let state_clone = state.clone();

    let _ipc_handle = tokio::spawn(async move {
        if let Err(e) = ipc::run_ipc_server(state_clone).await {
            tracing::error!("IPC server error: {}", e);
        }
    });

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let flag = shutdown_flag.clone();
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    tokio::spawn(async move {
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!("received SIGTERM");
            }
            _ = sigint.recv() => {
                tracing::info!("received SIGINT");
            }
        }
        flag.store(true, Ordering::Release);
    });

    tracing::info!("daemon ready, entering event loop (F1 mouse toggle, F8 capture, F9 pause)");

    let mut input_interval = tokio::time::interval(Duration::from_millis(1));
    input_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut tick_interval = tokio::time::interval(Duration::from_millis(16));
    tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        if shutdown_flag.load(Ordering::Acquire) {
            tracing::info!("shutdown signal received");
            break;
        }

        tokio::select! {
            _ = shutdown_rx.recv() => {
                tracing::info!("IPC shutdown command received");
                break;
            }
            _ = input_interval.tick() => {
                let raw_events = match ipc::lock_capture(&state) {
                    Ok(capture) => capture.poll_events(0),
                    Err(e) => {
                        tracing::warn!("input capture lock error: {}", e);
                        continue;
                    }
                };

                match raw_events {
                    Ok(raw_events) => {
                        if raw_events.is_empty() {
                            continue;
                        }

                        let input_events = match ipc::lock_capture(&state) {
                            Ok(mut capture) => capture.process_events(&raw_events),
                            Err(e) => {
                                tracing::warn!("input capture lock error: {}", e);
                                continue;
                            }
                        };
                        tracing::trace!(events = ?input_events, "translated input batch");

                        let mut gameplay_events = Vec::new();
                        for event in input_events {
                            match handle_runtime_shortcut(&state, &event).await {
                                Ok(true) => continue,
                                Ok(false) => {}
                                Err(e) => {
                                    tracing::warn!("runtime shortcut error: {}", e);
                                    continue;
                                }
                            }
                            if state.capture_active.load(Ordering::Acquire) {
                                gameplay_events.push(event);
                            } else {
                                tracing::trace!(event = ?event, "dropping gameplay event because capture is inactive");
                            }
                        }

                        if !gameplay_events.is_empty() {
                            tracing::trace!(events = ?gameplay_events, "forwarding gameplay events to engine");
                            let mut engine = state.engine.write().await;
                            let mut pending = Vec::new();
                            for event in &gameplay_events {
                                pending.extend(engine.process(event));
                            }
                            drop(engine);

                            if !pending.is_empty() {
                                let mut dev = match ipc::lock_touch_device(&state) {
                                    Ok(dev) => dev,
                                    Err(e) => {
                                        tracing::warn!("touch backend lock error: {}", e);
                                        continue;
                                    }
                                };
                                if let Err(e) = dev.apply_commands(&pending) {
                                    tracing::warn!("inject error: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("poll error: {}", e);
                    }
                }
            }
            _ = tick_interval.tick() => {
                if !state.capture_active.load(Ordering::Acquire) {
                    continue;
                }
                let mut engine = state.engine.write().await;
                let cmds = engine.tick();
                drop(engine);
                if !cmds.is_empty() {
                    let mut dev = match ipc::lock_touch_device(&state) {
                        Ok(dev) => dev,
                        Err(e) => {
                            tracing::warn!("touch backend lock error: {}", e);
                            continue;
                        }
                    };
                    if let Err(e) = dev.apply_commands(&cmds) {
                        tracing::warn!("inject error: {}", e);
                    }
                }
            }
        }
    }

    tracing::info!("performing clean shutdown...");

    {
        let mut engine = state.engine.write().await;
        let cmds = engine.release_all();
        drop(engine);
        let mut dev = ipc::lock_touch_device(&state)?;
        let _ = dev.apply_commands(&cmds);
    }

    {
        let mut capture = ipc::lock_capture(&state)?;
        capture.force_release_all();
    }

    let socket_path = config::socket_path();
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    tracing::info!("phantom shutdown complete");
    Ok(())
}

async fn handle_runtime_shortcut(state: &Arc<DaemonState>, event: &InputEvent) -> Result<bool> {
    match event {
        InputEvent::KeyPress(key) if *key == CAPTURE_TOGGLE_KEY => {
            let next = !state.capture_active.load(Ordering::Acquire);
            ipc::set_capture_active(state, next).await?;
            tracing::info!("capture {}", if next { "enabled" } else { "disabled" });
            Ok(true)
        }
        InputEvent::KeyPress(key) if *key == PAUSE_TOGGLE_KEY => {
            let cmds = {
                let mut engine = state.engine.write().await;
                if engine.is_paused() {
                    engine.resume();
                    tracing::info!("input processing resumed");
                    Vec::new()
                } else {
                    tracing::info!("input processing paused");
                    engine.pause()
                }
            };
            if !cmds.is_empty() {
                let mut dev = ipc::lock_touch_device(state)?;
                let _ = dev.apply_commands(&cmds);
            }
            Ok(true)
        }
        InputEvent::KeyPress(key) if *key == MOUSE_TOGGLE_KEY => {
            if !state.capture_active.load(Ordering::Acquire) {
                tracing::info!("F1 mouse toggle ignored (not in capture mode)");
                return Ok(true);
            }
            let mut capture = ipc::lock_capture(state)?;
            let currently_grabbed = capture.mouse_grabbed();
            if currently_grabbed {
                capture.set_grabbed_mouse_only(false)?;
                tracing::info!("mouse released to desktop (F1)");
            } else {
                capture.set_grabbed_mouse_only(true)?;
                tracing::info!("mouse grabbed for gameplay (F1)");
            }
            Ok(true)
        }
        InputEvent::KeyPress(key) if *key == RELEASE_ALL_KEY => {
            tracing::info!("F2: shutting down daemon");
            // Signal shutdown — cleanup happens in the main loop
            state.shutdown_tx.send(()).ok();
            Ok(true)
        }
        _ => Ok(false),
    }
}

async fn run_cli_command(args: &[String]) -> Result<()> {
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("status");

    if cmd == "audit" {
        let path = args.get(1).ok_or_else(|| {
            phantom::error::PhantomError::Ipc("audit requires a path argument".into())
        })?;
        let path = expand_path(path);
        let profile = Profile::load(std::path::Path::new(&path))?;
        print_profile_audit(&profile);
        return Ok(());
    }

    let request = match cmd {
        "load" => {
            let path = args.get(1).ok_or_else(|| {
                phantom::error::PhantomError::Ipc("load requires a path argument".into())
            })?;
            IpcRequest::LoadProfile { path: path.clone() }
        }
        "status" => IpcRequest::Status,
        "pause" => IpcRequest::Pause,
        "resume" => IpcRequest::Resume,
        "reload" => IpcRequest::Reload,
        "enter-capture" => IpcRequest::EnterCapture,
        "exit-capture" => IpcRequest::ExitCapture,
        "toggle-capture" => IpcRequest::ToggleCapture,
        "shutdown" => IpcRequest::Shutdown,
        "list" => IpcRequest::ListProfiles,
        "sensitivity" => {
            let value = args
                .get(1)
                .ok_or_else(|| {
                    phantom::error::PhantomError::Ipc("sensitivity requires a value".into())
                })?
                .parse::<f64>()
                .map_err(|_| {
                    phantom::error::PhantomError::Ipc("invalid sensitivity value".into())
                })?;
            IpcRequest::SetSensitivity { value }
        }
        other => {
            eprintln!("unknown command: {}", other);
            eprintln!("run 'phantom --help' for usage");
            std::process::exit(1);
        }
    };

    let response = ipc::send_command(&request).await?;

    if response.ok {
        if let Some(msg) = &response.message {
            eprintln!("{}", msg);
        }
        if let Some(profile) = &response.profile {
            eprintln!("profile: {}", profile);
        }
        if let Some(path) = &response.profile_path {
            eprintln!("profile path: {}", path);
        }
        if let (Some(w), Some(h)) = (response.screen_width, response.screen_height) {
            eprintln!("screen: {}x{}", w, h);
        }
        if let Some(nodes) = response.nodes {
            eprintln!("nodes: {}", nodes);
        }
        if let Some(slots) = &response.slots {
            eprintln!("slots: {}", format_slots(slots));
        }
        if let Some(paused) = response.paused {
            eprintln!("paused: {}", paused);
        }
        if let Some(capture_active) = response.capture_active {
            eprintln!("capture: {}", capture_active);
        }
        if let Some(sensitivity) = response.sensitivity {
            eprintln!("sensitivity: {}", sensitivity);
        }
        if let Some(active_layers) = &response.active_layers {
            if active_layers.is_empty() {
                eprintln!("active layers: (none)");
            } else {
                eprintln!("active layers: {}", active_layers.join(", "));
            }
        }
        if let Some(profiles) = &response.profiles {
            if profiles.is_empty() {
                eprintln!("no profiles found");
            } else {
                for p in profiles {
                    eprintln!("  {} — {}", p.name, p.path);
                }
            }
        }
    } else {
        if let Some(err) = &response.error {
            eprintln!("error: {}", err);
        }
        std::process::exit(1);
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let force_trace = args.iter().any(|a| a == "--trace");
    args.retain(|arg| arg != "--trace");
    init_logging(force_trace);

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        print_version();
        return;
    }

    if args.first().map(|s| s.as_str()) == Some("--daemon") {
        if let Err(e) = run_daemon().await {
            tracing::error!("daemon error: {}", e);
            std::process::exit(1);
        }
    } else if let Err(e) = run_cli_command(&args).await {
        tracing::error!("{}", e);
        std::process::exit(1);
    }
}
