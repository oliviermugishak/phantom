use std::env;
use std::ffi::c_char;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::error::{PhantomError, Result};
use crate::mouse_touch::{CursorSeed, HostFrame};

type Display = libc::c_void;
type Window = libc::c_ulong;
type Bool = libc::c_int;

const HELPER_SUBCOMMAND: &str = "__x11_cursor_helper";

#[link(name = "X11")]
unsafe extern "C" {
    fn XOpenDisplay(display_name: *const c_char) -> *mut Display;
    fn XCloseDisplay(display: *mut Display) -> libc::c_int;
    fn XDefaultScreen(display: *mut Display) -> libc::c_int;
    fn XRootWindow(display: *mut Display, screen_number: libc::c_int) -> Window;
    fn XQueryPointer(
        display: *mut Display,
        w: Window,
        root_return: *mut Window,
        child_return: *mut Window,
        root_x_return: *mut libc::c_int,
        root_y_return: *mut libc::c_int,
        win_x_return: *mut libc::c_int,
        win_y_return: *mut libc::c_int,
        mask_return: *mut libc::c_uint,
    ) -> Bool;
    fn XGetGeometry(
        display: *mut Display,
        d: Window,
        root_return: *mut Window,
        x_return: *mut libc::c_int,
        y_return: *mut libc::c_int,
        width_return: *mut libc::c_uint,
        height_return: *mut libc::c_uint,
        border_width_return: *mut libc::c_uint,
        depth_return: *mut libc::c_uint,
    ) -> libc::c_int;
    fn XTranslateCoordinates(
        display: *mut Display,
        src_w: Window,
        dest_w: Window,
        src_x: libc::c_int,
        src_y: libc::c_int,
        dest_x_return: *mut libc::c_int,
        dest_y_return: *mut libc::c_int,
        child_return: *mut Window,
    ) -> Bool;
}

#[derive(Debug)]
pub struct X11CursorClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl X11CursorClient {
    pub fn spawn() -> Result<Self> {
        let binary = env::current_exe().map_err(|e| {
            PhantomError::Internal(format!(
                "cannot locate phantom binary for x11 helper: {}",
                e
            ))
        })?;

        let mut command = Command::new(&binary);
        command
            .arg(HELPER_SUBCOMMAND)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        crate::session_env::apply_session_command_env(&mut command);

        let mut child = command.spawn().map_err(|e| {
            PhantomError::Internal(format!(
                "cannot launch x11 cursor helper via {}: {}",
                binary.display(),
                e
            ))
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| PhantomError::Internal("x11 cursor helper stdin unavailable".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| PhantomError::Internal("x11 cursor helper stdout unavailable".into()))?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    pub(crate) fn query_seed(&mut self) -> Result<Option<CursorSeed>> {
        self.stdin.write_all(b"cursor\n").map_err(|err| {
            PhantomError::Internal(format!("x11 cursor helper write failed: {}", err))
        })?;
        self.stdin.flush().map_err(|err| {
            PhantomError::Internal(format!("x11 cursor helper flush failed: {}", err))
        })?;

        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).map_err(|err| {
            PhantomError::Internal(format!("x11 cursor helper read failed: {}", err))
        })?;
        if read == 0 {
            return Err(PhantomError::Internal(
                "x11 cursor helper closed stdout".into(),
            ));
        }

        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("pos") => {
                let x = parse_seed_part(parts.next(), "x")?;
                let y = parse_seed_part(parts.next(), "y")?;
                let left = parse_seed_part(parts.next(), "left")?;
                let top = parse_seed_part(parts.next(), "top")?;
                let width = parse_seed_part(parts.next(), "width")?;
                let height = parse_seed_part(parts.next(), "height")?;
                Ok(Some(CursorSeed {
                    x,
                    y,
                    frame: HostFrame {
                        left,
                        top,
                        width,
                        height,
                    },
                }))
            }
            Some("none") => Ok(None),
            other => {
                tracing::warn!(reply = ?other, "x11 cursor helper returned an unexpected reply");
                Ok(None)
            }
        }
    }
}

impl Drop for X11CursorClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn run_helper_stdio() -> Result<()> {
    while let Some(line) = read_line()? {
        if line.trim() != "cursor" {
            continue;
        }

        if let Some(seed) = query_normalized_position() {
            println!(
                "pos {} {} {} {} {} {}",
                seed.x,
                seed.y,
                seed.frame.left,
                seed.frame.top,
                seed.frame.width,
                seed.frame.height
            );
        } else {
            println!("none");
        }
        std::io::stdout().flush().ok();
    }
    Ok(())
}

fn read_line() -> Result<Option<String>> {
    let mut line = String::new();
    let read = std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| PhantomError::Internal(format!("x11 helper stdin read failed: {}", e)))?;
    if read == 0 {
        return Ok(None);
    }
    Ok(Some(line))
}

fn query_normalized_position() -> Option<CursorSeed> {
    ensure_x11_env();
    let display = unsafe { XOpenDisplay(std::ptr::null()) };
    if display.is_null() {
        return None;
    }

    let root = unsafe { XRootWindow(display, XDefaultScreen(display)) };
    let result = unsafe {
        let (window, win_x, win_y) = top_level_window_under_pointer(display, root)?;
        let frame = window_frame(display, window)?;
        let x = (win_x as f64 / frame.width).clamp(0.0, 1.0);
        let y = (win_y as f64 / frame.height).clamp(0.0, 1.0);
        Some(CursorSeed { x, y, frame })
    };

    let _ = unsafe { XCloseDisplay(display) };
    result
}

unsafe fn top_level_window_under_pointer(
    display: *mut Display,
    root: Window,
) -> Option<(Window, libc::c_int, libc::c_int)> {
    let mut root_return = 0;
    let mut child_return = 0;
    let mut root_x = 0;
    let mut root_y = 0;
    let mut ignored_win_x = 0;
    let mut ignored_win_y = 0;
    let mut mask = 0;

    if XQueryPointer(
        display,
        root,
        &mut root_return,
        &mut child_return,
        &mut root_x,
        &mut root_y,
        &mut ignored_win_x,
        &mut ignored_win_y,
        &mut mask,
    ) == 0
    {
        return None;
    }

    if child_return == 0 {
        return None;
    }

    let window = child_return;
    let mut window_root_return = 0;
    let mut window_child_return = 0;
    let mut window_root_x = 0;
    let mut window_root_y = 0;
    let mut window_x = 0;
    let mut window_y = 0;
    let mut window_mask = 0;

    if XQueryPointer(
        display,
        window,
        &mut window_root_return,
        &mut window_child_return,
        &mut window_root_x,
        &mut window_root_y,
        &mut window_x,
        &mut window_y,
        &mut window_mask,
    ) == 0
    {
        return None;
    }

    Some((window, window_x, window_y))
}

unsafe fn window_frame(display: *mut Display, window: Window) -> Option<HostFrame> {
    let mut root_return = 0;
    let mut x = 0;
    let mut y = 0;
    let mut width = 0;
    let mut height = 0;
    let mut border = 0;
    let mut depth = 0;

    if XGetGeometry(
        display,
        window,
        &mut root_return,
        &mut x,
        &mut y,
        &mut width,
        &mut height,
        &mut border,
        &mut depth,
    ) == 0
    {
        return None;
    }

    if width == 0 || height == 0 {
        return None;
    }

    let mut dest_x = 0;
    let mut dest_y = 0;
    let mut child = 0;
    if XTranslateCoordinates(
        display,
        window,
        root_return,
        0,
        0,
        &mut dest_x,
        &mut dest_y,
        &mut child,
    ) == 0
    {
        dest_x = x;
        dest_y = y;
    }

    Some(HostFrame {
        left: dest_x as f64,
        top: dest_y as f64,
        width: width as f64,
        height: height as f64,
    })
}

fn parse_seed_part(value: Option<&str>, field: &str) -> Result<f64> {
    value
        .ok_or_else(|| PhantomError::Internal(format!("x11 seed missing {}", field)))?
        .parse::<f64>()
        .map_err(|_| PhantomError::Internal(format!("x11 seed has invalid {}", field)))
}

fn ensure_x11_env() {
    let runtime_dir = crate::session_env::preferred_runtime_dir();
    if env::var_os("DISPLAY").is_none() {
        if let Some(display) = crate::session_env::infer_display() {
            unsafe { env::set_var("DISPLAY", display) };
        }
    }

    if env::var_os("XAUTHORITY").is_none() {
        if let Some(xauthority) = crate::session_env::infer_xauthority(&runtime_dir) {
            unsafe { env::set_var("XAUTHORITY", xauthority) };
        }
    }
}
