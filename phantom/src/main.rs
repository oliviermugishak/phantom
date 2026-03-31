use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tracing_subscriber::EnvFilter;

use phantom::config;
use phantom::engine::{self, KeymapEngine};
use phantom::error::Result;
use phantom::inject::UinputDevice;
use phantom::input::InputCapture;
use phantom::ipc::{self, DaemonState, IpcRequest};
use phantom::profile::Profile;

fn print_help() {
    eprintln!(
        r#"phantom — virtual touchscreen for Waydroid

USAGE:
    phantom --daemon              Start the daemon
    phantom load <profile.json>   Load a profile
    phantom status                Show daemon status
    phantom pause                 Pause input processing
    phantom resume                Resume input processing
    phantom reload                Reload current profile
    phantom sensitivity <value>   Set global sensitivity
    phantom list                  List available profiles
    phantom shutdown              Graceful shutdown

FLAGS:
    --daemon      Run as daemon (requires root)
    -h, --help    Show this help
    -V, --version Show version"#
    );
}

fn print_version() {
    eprintln!("phantom {}", env!("CARGO_PKG_VERSION"));
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .init();
}

/// Detect screen resolution from framebuffer.
fn detect_resolution(config: &config::Config) -> Result<(u32, u32)> {
    // Config override takes priority
    if let (Some(w), Some(h)) = (config.screen.width, config.screen.height) {
        tracing::info!("screen resolution from config: {}x{}", w, h);
        return Ok((w, h));
    }

    // Try /sys/class/graphics/fb0/virtual_size
    let sysfs_path = "/sys/class/graphics/fb0/virtual_size";
    if let Ok(content) = std::fs::read_to_string(sysfs_path) {
        let parts: Vec<&str> = content.trim().split(',').collect();
        if parts.len() == 2 {
            if let (Ok(w), Ok(h)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                if w > 0 && h > 0 {
                    tracing::info!("screen resolution from {}: {}x{}", sysfs_path, w, h);
                    return Ok((w, h));
                }
            }
        }
    }

    // Try /dev/fb0 ioctl
    if let Ok(file) = std::fs::File::open("/dev/fb0") {
        use std::os::unix::io::AsRawFd;
        #[repr(C)]
        struct FbVarScreeninfo {
            xres: u32, yres: u32,
            xres_virtual: u32, yres_virtual: u32,
            xoffset: u32, yoffset: u32,
            bits_per_pixel: u32,
            grayscale: u32,
            red: [u32; 5], green: [u32; 5], blue: [u32; 5], transp: [u32; 5],
            nonstd: u32, activate: u32, height: u32, width: u32,
            accel_flags: u32,
            pixclock: u32, left_margin: u32, right_margin: u32,
            upper_margin: u32, lower_margin: u32, hsync_len: u32, vsync_len: u32,
            sync: u32, vmode: u32, rotate: u32, colorspace: u32,
            reserved: [u32; 4],
        }
        let mut info: FbVarScreeninfo = unsafe { std::mem::zeroed() };
        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), 0x4600 /* FBIOGET_VSCREENINFO */, &mut info)
        };
        if ret >= 0 && info.xres > 0 && info.yres > 0 {
            tracing::info!("screen resolution from /dev/fb0: {}x{}", info.xres, info.yres);
            return Ok((info.xres, info.yres));
        }
    }

    // Default fallback
    tracing::warn!("could not detect screen resolution, using 1920x1080");
    Ok((1920, 1080))
}

async fn run_daemon() -> Result<()> {
    tracing::info!("phantom {} starting", env!("CARGO_PKG_VERSION"));

    // Load config
    let config = config::load_config();
    tracing::info!("config loaded");

    // Detect resolution
    let (screen_w, screen_h) = detect_resolution(&config)?;

    // Create uinput device
    let uinput = UinputDevice::new(screen_w, screen_h)?;

    // Discover and grab input devices
    let capture = InputCapture::discover_and_grab()?;
    tracing::info!(
        devices = capture.device_count(),
        mouse = capture.has_mouse(),
        keyboard = capture.has_keyboard(),
        "input capture ready"
    );

    // Load default profile or create empty engine
    let profile = config::default_profile_path()
        .and_then(|p| {
            tracing::info!("loading default profile: {}", p.display());
            Profile::load(&p).ok()
        });

    let engine = match profile {
        Some(p) => KeymapEngine::new(p),
        None => {
            tracing::warn!("no default profile found, engine idle until profile loaded via IPC");
            // Create a minimal empty profile
            KeymapEngine::new(phantom::profile::Profile {
                name: "empty".into(),
                version: 1,
                screen: None,
                global_sensitivity: 1.0,
                nodes: vec![],
            })
        }
    };

    // Set up daemon state and IPC
    let (state, mut shutdown_rx) = DaemonState::new(engine, screen_w, screen_h);
    let state_clone = state.clone();

    // Spawn IPC server
    let _ipc_handle = tokio::spawn(async move {
        if let Err(e) = ipc::run_ipc_server(state_clone).await {
            tracing::error!("IPC server error: {}", e);
        }
    });

    // Set up signal handling
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let flag = shutdown_flag.clone();
    tokio::spawn(async move {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .expect("failed to install SIGINT handler");
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

    // Convert uinput to shared mutable reference
    let uinput = Arc::new(std::sync::Mutex::new(uinput));

    tracing::info!("daemon ready, entering event loop");

    // Main event loop
    let mut tick_interval = tokio::time::interval(Duration::from_millis(16)); // ~60Hz tick

    loop {
        // Check for shutdown
        if shutdown_flag.load(Ordering::Acquire) {
            tracing::info!("shutdown signal received");
            break;
        }

        tokio::select! {
            // Check for IPC shutdown
            _ = shutdown_rx.recv() => {
                tracing::info!("IPC shutdown command received");
                break;
            }

            // Poll for input events
            _ = tick_interval.tick() => {
                // Poll input devices (non-blocking)
                match capture.poll_events(0) {
                    Ok(raw_events) => {
                        if !raw_events.is_empty() {
                            let input_events = capture.process_events(&raw_events);
                            let mut engine = state.engine.write().await;
                            for event in &input_events {
                                let cmds = engine.process(event);
                                if !cmds.is_empty() {
                                    let mut dev = uinput.lock().unwrap();
                                    if let Err(e) = engine::execute_commands(&mut dev, &cmds) {
                                        tracing::warn!("inject error: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("poll error: {}", e);
                    }
                }

                // Tick engine for time-based nodes (repeat_tap, macros)
                let mut engine = state.engine.write().await;
                let cmds = engine.tick();
                if !cmds.is_empty() {
                    let mut dev = uinput.lock().unwrap();
                    if let Err(e) = engine::execute_commands(&mut dev, &cmds) {
                        tracing::warn!("inject error: {}", e);
                    }
                }
            }
        }
    }

    // Clean shutdown
    tracing::info!("performing clean shutdown...");

    // Release all touches
    {
        let mut engine = state.engine.write().await;
        let cmds = engine.release_all();
        let mut dev = uinput.lock().unwrap();
        let _ = engine::execute_commands(&mut dev, &cmds);
    }

    // Clean up socket
    let socket_path = config::socket_path();
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    // uinput device is destroyed via Drop
    // InputCapture grabs are released via Drop
    // IPC listener is dropped when tokio runtime shuts down

    tracing::info!("phantom shutdown complete");
    Ok(())
}

async fn run_cli_command(args: &[String]) -> Result<()> {
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("status");

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
        "shutdown" => IpcRequest::Shutdown,
        "list" => IpcRequest::ListProfiles,
        "sensitivity" => {
            let value = args.get(1).ok_or_else(|| {
                phantom::error::PhantomError::Ipc("sensitivity requires a value".into())
            })?.parse::<f64>().map_err(|_| {
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

    // Pretty-print response
    if response.ok {
        if let Some(msg) = &response.message {
            eprintln!("{}", msg);
        }
        if let Some(profile) = &response.profile {
            eprintln!("profile: {}", profile);
        }
        if let Some(paused) = response.paused {
            eprintln!("paused: {}", paused);
        }
        if let Some(sensitivity) = response.sensitivity {
            eprintln!("sensitivity: {}", sensitivity);
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
    init_logging();

    let args: Vec<String> = std::env::args().skip(1).collect();

    // Handle flags
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
    } else {
        if let Err(e) = run_cli_command(&args).await {
            tracing::error!("{}", e);
            std::process::exit(1);
        }
    }
}
