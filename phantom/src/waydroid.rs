use crate::config::Config;
use crate::error::{PhantomError, Result};
use crate::inject::{PHANTOM_DEVICE_NAME, PHANTOM_PRODUCT_ID, PHANTOM_VENDOR_ID};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const WAYDROID_DEFAULT_WORK_DIR: &str = "/var/lib/waydroid";
const DEFAULT_ANDROID_SERVER_JAR_CONTAINER_PATH: &str = "/data/local/tmp/phantom-server.jar";
const DEFAULT_ANDROID_SERVER_LOG_CONTAINER_PATH: &str = "/data/local/tmp/phantom-server.log";
const DEFAULT_ANDROID_SERVER_BIND_HOST: &str = "0.0.0.0";
const DEFAULT_ANDROID_SERVER_PORT: u16 = 27183;
const IDC_TEMPLATE: &str = include_str!("../../contrib/waydroid/Vendor_1234_Product_5678.idc");

#[derive(Debug, Clone)]
pub struct WaydroidPaths {
    pub work_dir: PathBuf,
    pub overlay_dir: PathBuf,
    pub idc_dir: PathBuf,
    pub vendor_product_idc: PathBuf,
    pub device_name_idc: PathBuf,
    pub waydroid_cfg: PathBuf,
}

#[derive(Debug, Clone)]
pub struct InstallReport {
    pub paths: WaydroidPaths,
    pub files_written: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct CommandReport {
    pub command: String,
    pub ok: bool,
    pub output: String,
}

#[derive(Debug, Clone)]
pub struct DiagnosisReport {
    pub paths: WaydroidPaths,
    pub mount_overlays: Option<bool>,
    pub idc_vendor_exists: bool,
    pub idc_name_exists: bool,
    pub getevent: CommandReport,
    pub dumpsys: CommandReport,
}

pub fn waydroid_work_dir(config: &Config) -> PathBuf {
    config
        .waydroid
        .work_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(WAYDROID_DEFAULT_WORK_DIR))
}

pub fn android_server_host(config: &Config) -> Result<String> {
    if let Some(host) = config.android.host.as_ref() {
        return Ok(host.clone());
    }

    let status = waydroid_status_output()?;
    parse_waydroid_status_ip(&status).ok_or_else(|| {
        PhantomError::TouchBackend(format!(
            "could not determine Waydroid container IP from 'waydroid status':\n{}",
            status
        ))
    })
}

pub fn android_server_port(config: &Config) -> u16 {
    config.android.port.unwrap_or(DEFAULT_ANDROID_SERVER_PORT)
}

pub fn android_server_bind_host(config: &Config) -> String {
    config
        .android
        .container_bind_host
        .clone()
        .unwrap_or_else(|| DEFAULT_ANDROID_SERVER_BIND_HOST.into())
}

pub fn android_server_log_container_path(config: &Config) -> String {
    config
        .android
        .container_log_path
        .clone()
        .unwrap_or_else(|| DEFAULT_ANDROID_SERVER_LOG_CONTAINER_PATH.into())
}

pub fn android_server_jar_container_path(config: &Config) -> String {
    config
        .android
        .container_server_jar
        .clone()
        .unwrap_or_else(|| DEFAULT_ANDROID_SERVER_JAR_CONTAINER_PATH.into())
}

pub fn ensure_android_server(config: &Config) -> Result<()> {
    ensure_waydroid_session_running()?;

    let staged_jar = stage_android_server(config)?;
    let container_jar = android_server_jar_container_path(config);
    let bind_host = android_server_bind_host(config);
    let port = android_server_port(config);
    let log_path = android_server_log_container_path(config);
    let server_class = config.android.server_class.trim();
    if server_class.is_empty() {
        return Err(PhantomError::TouchBackend(
            "android server class cannot be empty".into(),
        ));
    }

    tracing::info!(
        staged_jar = %staged_jar.display(),
        host = bind_host,
        port = port,
        server_class = server_class,
        "launching android touch server"
    );

    let shell = format!(
        "rm -f {log}; CLASSPATH={jar} app_process / {class} --host {host} --port {port} </dev/null >{log} 2>&1 &",
        jar = sh_quote(&container_jar),
        class = sh_quote(server_class),
        host = sh_quote(&bind_host),
        port = port,
        log = sh_quote(&log_path),
    );

    let output = Command::new("waydroid")
        .arg("shell")
        .arg("--")
        .arg("sh")
        .arg("-c")
        .arg(&shell)
        .output()
        .map_err(|e| {
            PhantomError::TouchBackend(format!("failed to launch android touch server: {}", e))
        })?;

    if !output.status.success() {
        let detail = command_output_detail(&output);

        return Err(PhantomError::TouchBackend(format!(
            "waydroid shell failed to launch android touch server: {}",
            detail
        )));
    }

    Ok(())
}

pub fn android_server_log_excerpt(config: &Config) -> Option<String> {
    let log_path = android_server_log_container_path(config);
    let shell = format!("tail -n 80 {}", sh_quote(&log_path));
    let output = Command::new("waydroid")
        .arg("shell")
        .arg("--")
        .arg("sh")
        .arg("-c")
        .arg(&shell)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let detail = command_output_detail(&output);
    (!detail.trim().is_empty()).then_some(detail)
}

pub fn phantom_idc_text() -> &'static str {
    IDC_TEMPLATE
}

pub fn phantom_paths(work_dir: impl AsRef<Path>) -> WaydroidPaths {
    let work_dir = work_dir.as_ref().to_path_buf();
    let overlay_dir = work_dir.join("overlay");
    let idc_dir = overlay_dir.join("usr").join("idc");
    WaydroidPaths {
        waydroid_cfg: work_dir.join("waydroid.cfg"),
        vendor_product_idc: idc_dir.join(vendor_product_idc_filename()),
        device_name_idc: idc_dir.join(device_name_idc_filename(PHANTOM_DEVICE_NAME)),
        work_dir,
        overlay_dir,
        idc_dir,
    }
}

pub fn install_phantom_idc(work_dir: impl AsRef<Path>) -> Result<InstallReport> {
    let paths = phantom_paths(work_dir);
    std::fs::create_dir_all(&paths.idc_dir).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            PhantomError::PermissionDenied {
                path: paths.idc_dir.display().to_string(),
                reason: "run this command with sudo or write to the Waydroid overlay manually"
                    .into(),
            }
        } else {
            PhantomError::Io(e)
        }
    })?;

    for path in [&paths.vendor_product_idc, &paths.device_name_idc] {
        std::fs::write(path, IDC_TEMPLATE).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                PhantomError::PermissionDenied {
                    path: path.display().to_string(),
                    reason: "run this command with sudo or write to the Waydroid overlay manually"
                        .into(),
                }
            } else {
                PhantomError::Io(e)
            }
        })?;
    }

    Ok(InstallReport {
        files_written: vec![
            paths.vendor_product_idc.clone(),
            paths.device_name_idc.clone(),
        ],
        paths,
    })
}

pub fn diagnose_phantom_input(work_dir: impl AsRef<Path>) -> DiagnosisReport {
    let paths = phantom_paths(work_dir);
    DiagnosisReport {
        mount_overlays: read_mount_overlays(&paths.waydroid_cfg),
        idc_vendor_exists: paths.vendor_product_idc.exists(),
        idc_name_exists: paths.device_name_idc.exists(),
        getevent: run_waydroid_shell(["getevent", "-lp"], PHANTOM_DEVICE_NAME, 20),
        dumpsys: run_waydroid_shell(["dumpsys", "input"], PHANTOM_DEVICE_NAME, 40),
        paths,
    }
}

pub fn render_install_report(report: &InstallReport) -> String {
    format!(
        "Installed Waydroid IDC files:\n- {}\n- {}\n\nWaydroid overlay root: {}\nRestart the Waydroid session after this change.",
        report.files_written[0].display(),
        report.files_written[1].display(),
        report.paths.overlay_dir.display()
    )
}

pub fn render_diagnosis(report: &DiagnosisReport) -> String {
    let mount_overlays = match report.mount_overlays {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    };

    format!(
        "Waydroid work dir: {}\nOverlay dir: {}\nmount_overlays: {}\nIDC present (vendor/product): {}\nIDC present (device name): {}\n\n== getevent -lp ==\n{}\n\n== dumpsys input ==\n{}",
        report.paths.work_dir.display(),
        report.paths.overlay_dir.display(),
        mount_overlays,
        yes_no(report.idc_vendor_exists),
        yes_no(report.idc_name_exists),
        render_command_report(&report.getevent),
        render_command_report(&report.dumpsys),
    )
}

fn render_command_report(report: &CommandReport) -> String {
    if report.ok {
        report.output.clone()
    } else {
        format!("{} failed:\n{}", report.command, report.output)
    }
}

fn run_waydroid_shell<const N: usize>(
    args: [&str; N],
    needle: &str,
    context_after: usize,
) -> CommandReport {
    let inner = args.join(" ");
    let command = format!("waydroid shell sh -c '{}'", inner);
    let mut cmd = Command::new("waydroid");
    cmd.arg("shell");
    cmd.arg("--");
    cmd.arg("sh");
    cmd.arg("-c");
    cmd.arg(&inner);
    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = if stderr.trim().is_empty() {
                stdout.to_string()
            } else if stdout.trim().is_empty() {
                stderr.to_string()
            } else {
                format!("{}\n{}", stdout, stderr)
            };

            let excerpt = excerpt_with_context(&combined, needle, context_after)
                .unwrap_or_else(|| truncate_lines(&combined, 120));
            CommandReport {
                command,
                ok: output.status.success(),
                output: excerpt,
            }
        }
        Err(e) => CommandReport {
            command,
            ok: false,
            output: e.to_string(),
        },
    }
}

fn excerpt_with_context(text: &str, needle: &str, context_after: usize) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let idx = lines.iter().position(|line| line.contains(needle))?;
    let start = idx.saturating_sub(3);
    let end = (idx + context_after).min(lines.len());
    Some(lines[start..end].join("\n"))
}

fn truncate_lines(text: &str, limit: usize) -> String {
    let mut lines = text.lines();
    let mut rendered = Vec::new();
    for _ in 0..limit {
        let Some(line) = lines.next() else {
            break;
        };
        rendered.push(line);
    }
    if lines.next().is_some() {
        rendered.push("...");
    }
    rendered.join("\n")
}

fn read_mount_overlays(path: &Path) -> Option<bool> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("mount_overlays") {
            let Some((_, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches('"');
            if value.eq_ignore_ascii_case("true") {
                return Some(true);
            }
            if value.eq_ignore_ascii_case("false") {
                return Some(false);
            }
        }
    }
    None
}

fn stage_android_server(config: &Config) -> Result<PathBuf> {
    let source = config.android.server_jar.clone().ok_or_else(|| {
        PhantomError::TouchBackend(
            "android auto-launch requires [android].server_jar to point to a built phantom-server.jar".into(),
        )
    })?;

    if !source.exists() {
        return Err(PhantomError::TouchBackend(format!(
            "android server jar not found at {}",
            source.display()
        )));
    }

    validate_android_server_jar(&source)?;

    let bytes = std::fs::read(&source).map_err(|e| {
        PhantomError::TouchBackend(format!(
            "cannot read android server jar {}: {}",
            source.display(),
            e
        ))
    })?;
    let container_jar = android_server_jar_container_path(config);
    let shell = format!(
        "cat >{jar} && chmod 0644 {jar}",
        jar = sh_quote(&container_jar)
    );
    let mut child = Command::new("waydroid")
        .arg("shell")
        .arg("--")
        .arg("sh")
        .arg("-c")
        .arg(&shell)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            PhantomError::TouchBackend(format!("failed to stage android server jar: {}", e))
        })?;

    let mut stdin = child.stdin.take().ok_or_else(|| {
        PhantomError::TouchBackend("failed to open stdin for waydroid shell staging".into())
    })?;
    stdin.write_all(&bytes).map_err(|e| {
        PhantomError::TouchBackend(format!(
            "failed writing android server jar into container: {}",
            e
        ))
    })?;
    drop(stdin);

    let output = child.wait_with_output().map_err(|e| {
        PhantomError::TouchBackend(format!("failed waiting for android server staging: {}", e))
    })?;
    if !output.status.success() {
        return Err(PhantomError::TouchBackend(format!(
            "failed staging android server jar in container: {}",
            command_output_detail(&output)
        )));
    }

    Ok(source)
}

fn ensure_waydroid_session_running() -> Result<()> {
    let detail = waydroid_status_output()?;
    if !waydroid_status_is_running(&detail) {
        return Err(PhantomError::TouchBackend(format!(
            "Waydroid session is not running; run 'waydroid session start' before starting Phantom with android_socket. Current status:\n{}",
            detail
        )));
    }
    if waydroid_container_is_frozen(&detail) {
        return Err(PhantomError::TouchBackend(format!(
            "Waydroid container is frozen; open Waydroid with 'waydroid show-full-ui' or launch the game before starting Phantom with android_socket. Current status:\n{}",
            detail
        )));
    }

    Ok(())
}

fn waydroid_status_output() -> Result<String> {
    let output = Command::new("waydroid")
        .arg("status")
        .output()
        .map_err(|e| {
            PhantomError::TouchBackend(format!("failed to query waydroid status: {}", e))
        })?;

    if !output.status.success() {
        return Err(PhantomError::TouchBackend(format!(
            "waydroid status failed: {}",
            command_output_detail(&output)
        )));
    }

    Ok(command_output_detail(&output))
}

fn parse_waydroid_status_ip(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let line = line.trim();
        let value = line.strip_prefix("IP address:")?.trim();
        (!value.is_empty()).then_some(value.to_string())
    })
}

fn validate_android_server_jar(path: &Path) -> Result<()> {
    let bytes = std::fs::read(path).map_err(|e| {
        PhantomError::TouchBackend(format!(
            "cannot read android server jar {}: {}",
            path.display(),
            e
        ))
    })?;

    if !bytes
        .windows(b"classes.dex".len())
        .any(|w| w == b"classes.dex")
    {
        return Err(PhantomError::TouchBackend(format!(
            "android server jar {} is not dexed (missing classes.dex); rebuild it with contrib/android-server/build.sh",
            path.display()
        )));
    }

    Ok(())
}

fn vendor_product_idc_filename() -> String {
    format!(
        "Vendor_{:04x}_Product_{:04x}.idc",
        PHANTOM_VENDOR_ID, PHANTOM_PRODUCT_ID
    )
}

fn command_output_detail(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        format!("{}\n{}", stdout.trim(), stderr.trim())
    }
}

fn waydroid_status_is_running(output: &str) -> bool {
    output
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_prefix("Session:")
                .map(|value| value.trim().eq_ignore_ascii_case("RUNNING"))
        })
        .unwrap_or(false)
}

fn waydroid_container_is_frozen(output: &str) -> bool {
    output
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_prefix("Container:")
                .map(|value| value.trim().eq_ignore_ascii_case("FROZEN"))
        })
        .unwrap_or(false)
}

fn device_name_idc_filename(name: &str) -> String {
    let sanitized: OsString = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .into();
    PathBuf::from(sanitized)
        .with_extension("idc")
        .display()
        .to_string()
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn sh_quote(value: &str) -> String {
    let mut rendered = String::from("'");
    for ch in value.chars() {
        if ch == '\'' {
            rendered.push_str("'\"'\"'");
        } else {
            rendered.push(ch);
        }
    }
    rendered.push('\'');
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_product_file_name_matches_android_lookup() {
        assert_eq!(
            vendor_product_idc_filename(),
            "Vendor_1234_Product_5678.idc"
        );
    }

    #[test]
    fn device_name_file_name_is_sanitized() {
        assert_eq!(
            device_name_idc_filename("Phantom Virtual Touch"),
            "Phantom_Virtual_Touch.idc"
        );
    }

    #[test]
    fn overlay_paths_are_resolved_under_work_dir() {
        let paths = phantom_paths("/tmp/waydroid");
        assert_eq!(paths.overlay_dir, PathBuf::from("/tmp/waydroid/overlay"));
        assert_eq!(
            paths.vendor_product_idc,
            PathBuf::from("/tmp/waydroid/overlay/usr/idc/Vendor_1234_Product_5678.idc")
        );
    }

    #[test]
    fn excerpt_finds_matching_block() {
        let text = "a\nb\nPhantom Virtual Touch\nc\nd";
        let excerpt = excerpt_with_context(text, "Phantom Virtual Touch", 1).unwrap();
        assert!(excerpt.contains("Phantom Virtual Touch"));
        assert!(excerpt.contains("c"));
    }

    #[test]
    fn idc_text_contains_touchscreen_classification() {
        let text = phantom_idc_text();
        assert!(text.contains("touch.deviceType = touchScreen"));
        assert!(text.contains("touch.orientationAware = 1"));
    }

    #[test]
    fn android_server_defaults_are_tcp_and_container_local_tmp() {
        let cfg = Config::default();
        assert_eq!(android_server_port(&cfg), 27183);
        assert_eq!(android_server_bind_host(&cfg), "0.0.0.0");
        assert_eq!(
            android_server_jar_container_path(&cfg),
            "/data/local/tmp/phantom-server.jar"
        );
    }

    #[test]
    fn waydroid_status_parser_accepts_running() {
        assert!(waydroid_status_is_running(
            "Session:\tRUNNING\nVendor type:\tMAINLINE\n"
        ));
    }

    #[test]
    fn waydroid_status_parser_rejects_stopped() {
        assert!(!waydroid_status_is_running(
            "Session:\tSTOPPED\nVendor type:\tMAINLINE\n"
        ));
    }

    #[test]
    fn waydroid_status_parser_extracts_ip() {
        assert_eq!(
            parse_waydroid_status_ip(
                "Session:\tRUNNING\nIP address:\t192.168.240.112\nVendor type:\tMAINLINE\n"
            ),
            Some("192.168.240.112".into())
        );
    }

    #[test]
    fn waydroid_status_parser_detects_frozen_container() {
        assert!(waydroid_container_is_frozen(
            "Session:\tRUNNING\nContainer:\tFROZEN\n"
        ));
        assert!(!waydroid_container_is_frozen(
            "Session:\tRUNNING\nContainer:\tRUNNING\n"
        ));
    }
}
