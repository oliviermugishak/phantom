use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;
use crate::error::{PhantomError, Result};
use crate::inject::{PHANTOM_DEVICE_NAME, PHANTOM_PRODUCT_ID, PHANTOM_VENDOR_ID};

const WAYDROID_DEFAULT_WORK_DIR: &str = "/var/lib/waydroid";
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

fn vendor_product_idc_filename() -> String {
    format!(
        "Vendor_{:04x}_Product_{:04x}.idc",
        PHANTOM_VENDOR_ID, PHANTOM_PRODUCT_ID
    )
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
}
