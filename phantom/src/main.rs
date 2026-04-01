use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tracing_subscriber::EnvFilter;

use phantom::config;
use phantom::desktop_relay::DesktopKeyboardRelay;
use phantom::engine::KeymapEngine;
use phantom::error::{PhantomError, Result};
use phantom::input::{InputCapture, InputEvent};
use phantom::ipc::{self, DaemonState, IpcRequest};
use phantom::profile::Profile;
use phantom::touch;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!(
        r#"{app_name} {app_version}

Virtual touchscreen for Waydroid

USAGE:
    {app_name} --daemon                  Start the daemon
    {app_name} version                   Show version
    {app_name} audit <profile.json>      Audit slot and binding usage in a profile
    {app_name} load <profile.json>       Load a profile
    {app_name} status                    Show daemon status
    {app_name} pause                     Pause input processing
    {app_name} resume                    Resume input processing
    {app_name} reload                    Reload current profile
    {app_name} enter-capture             Enable exclusive gameplay capture
    {app_name} exit-capture              Release exclusive gameplay capture
    {app_name} toggle-capture            Toggle gameplay capture
    {app_name} grab-mouse                Route mouse input into the game
    {app_name} release-mouse             Release mouse input back to desktop
    {app_name} toggle-mouse              Toggle mouse routing while capture is active
    {app_name} sensitivity <value>       Set global sensitivity
    {app_name} list                      List available profiles
    {app_name} shutdown                  Graceful shutdown

KEYS (while daemon running, configurable in config.toml [runtime_hotkeys]):
    F2   Shutdown daemon (default)
    F1   Toggle mouse grab (default)
    F8   Toggle capture mode (default)
    F9   Toggle pause (default)

FLAGS:
    --daemon      Run as daemon (requires root)
    --trace       Force trace logging for this run
    -h, --help    Show this help
    -V, --version Show version"#,
        app_name = APP_NAME,
        app_version = APP_VERSION,
    );
}

fn print_version() {
    println!("{} {}", APP_NAME, APP_VERSION);
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
    let runtime_hotkeys = config::resolved_runtime_hotkeys(&config);
    let touch = touch::create_touch_device(&config, screen_w, screen_h)?;
    let desktop_keyboard = DesktopKeyboardRelay::new()?;
    let mut capture = InputCapture::discover()?;
    capture.set_grabbed_keyboard_only(true)?;
    tracing::info!("keyboard grab enabled for runtime hotkeys");
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

    let (state, mut shutdown_rx) =
        DaemonState::new(engine, touch, desktop_keyboard, capture, screen_w, screen_h);
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

    tracing::info!(
        mouse_toggle = ?runtime_hotkeys.mouse_toggle,
        capture_toggle = ?runtime_hotkeys.capture_toggle,
        pause_toggle = ?runtime_hotkeys.pause_toggle,
        shutdown = ?runtime_hotkeys.shutdown,
        "daemon ready, entering event loop"
    );

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
                        let (mut mouse_routed, mut keyboard_routed) = match ipc::lock_capture(&state)
                        {
                            Ok(capture) => (capture.mouse_grabbed(), capture.keyboard_grabbed()),
                            Err(e) => {
                                tracing::warn!("input capture lock error: {}", e);
                                continue;
                            }
                        };
                        for event in input_events {
                            match handle_runtime_shortcut(&state, &event, &runtime_hotkeys).await {
                                Ok(true) => {
                                    match ipc::lock_capture(&state) {
                                        Ok(capture) => {
                                            mouse_routed = capture.mouse_grabbed();
                                            keyboard_routed = capture.keyboard_grabbed();
                                        }
                                        Err(e) => {
                                            tracing::warn!("input capture lock error: {}", e);
                                        }
                                    }
                                    continue;
                                }
                                Ok(false) => {}
                                Err(e) => {
                                    tracing::warn!("runtime shortcut error: {}", e);
                                    continue;
                                }
                            }
                            if state.capture_active.load(Ordering::Acquire) {
                                // Capture and routing are intentionally separate.
                                // We may still be in gameplay capture while the user has
                                // temporarily released only the mouse back to the desktop.
                                if event.is_mouse_input() && !mouse_routed {
                                    tracing::trace!(
                                        event = ?event,
                                        "dropping gameplay event because mouse routing is disabled"
                                    );
                                    continue;
                                }
                                if event.is_keyboard_input() && !keyboard_routed {
                                    tracing::trace!(
                                        event = ?event,
                                        "dropping gameplay event because keyboard routing is disabled"
                                    );
                                    continue;
                                }
                                gameplay_events.push(event);
                            } else {
                                if event.is_keyboard_input() {
                                    if let Err(e) = ipc::relay_keyboard_event_to_desktop(&state, &event) {
                                        tracing::warn!("desktop keyboard relay error: {}", e);
                                    }
                                }
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
        if let Ok(mut relay) = ipc::lock_desktop_keyboard(&state) {
            let _ = relay.release_all();
        }
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

async fn handle_runtime_shortcut(
    state: &Arc<DaemonState>,
    event: &InputEvent,
    hotkeys: &config::RuntimeHotkeys,
) -> Result<bool> {
    if let InputEvent::KeyRelease(key) = event {
        if is_runtime_hotkey(*key, hotkeys) {
            return Ok(true);
        }
    }

    match event {
        InputEvent::KeyPress(key) if hotkeys.capture_toggle == Some(*key) => {
            tracing::info!(?key, "runtime hotkey pressed");
            let next = !state.capture_active.load(Ordering::Acquire);
            ipc::set_capture_active(state, next).await?;
            tracing::info!("capture {}", if next { "enabled" } else { "disabled" });
            Ok(true)
        }
        InputEvent::KeyPress(key) if hotkeys.pause_toggle == Some(*key) => {
            tracing::info!(?key, "runtime hotkey pressed");
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
        InputEvent::KeyPress(key) if hotkeys.mouse_toggle == Some(*key) => {
            tracing::info!(?key, "runtime hotkey pressed");
            if !state.capture_active.load(Ordering::Acquire) {
                tracing::info!("mouse toggle ignored (not in capture mode)");
                return Ok(true);
            }
            let currently_grabbed = {
                let capture = ipc::lock_capture(state)?;
                capture.mouse_grabbed()
            };
            ipc::set_mouse_routed(state, !currently_grabbed).await?;
            tracing::info!(
                "{}",
                if currently_grabbed {
                    "mouse released to desktop"
                } else {
                    "mouse grabbed for gameplay"
                }
            );
            Ok(true)
        }
        InputEvent::KeyPress(key) if hotkeys.shutdown == Some(*key) => {
            tracing::info!(?key, "runtime hotkey pressed");
            // Signal shutdown — cleanup happens in the main loop
            state.shutdown_tx.send(()).ok();
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn is_runtime_hotkey(key: phantom::input::Key, hotkeys: &config::RuntimeHotkeys) -> bool {
    hotkeys.mouse_toggle == Some(key)
        || hotkeys.capture_toggle == Some(key)
        || hotkeys.pause_toggle == Some(key)
        || hotkeys.shutdown == Some(key)
}

async fn run_cli_command(args: &[String]) -> Result<()> {
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("status");

    match cmd {
        "version" => {
            print_version();
            return Ok(());
        }
        "audit" => {
            let path = args.get(1).ok_or_else(|| {
                phantom::error::PhantomError::Ipc("audit requires a path argument".into())
            })?;
            let path = expand_path(path);
            let profile = Profile::load(std::path::Path::new(&path))?;
            print_profile_audit(&profile);
            return Ok(());
        }
        _ => {}
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
        "grab-mouse" => IpcRequest::GrabMouse,
        "release-mouse" => IpcRequest::ReleaseMouse,
        "toggle-mouse" => IpcRequest::ToggleMouse,
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
        if let Some(mouse_grabbed) = response.mouse_grabbed {
            eprintln!("mouse grabbed: {}", mouse_grabbed);
        }
        if let Some(keyboard_grabbed) = response.keyboard_grabbed {
            eprintln!("keyboard grabbed: {}", keyboard_grabbed);
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

    if args.first().map(|s| s.as_str()) == Some("help")
        || args.iter().any(|a| a == "-h" || a == "--help")
    {
        print_help();
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("version")
        || args.iter().any(|a| a == "-V" || a == "--version")
    {
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
