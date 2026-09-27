//! Fixed Krita CLI source-to-PNG export. No caller-selected executable or script.

use serde::Serialize;
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MAX_SOURCE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_EXPORT_BYTES: u64 = 512 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(90);
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportStatus {
    Exported,
    Unavailable,
    Incomplete,
    InvalidInput,
    ToolError,
}

#[derive(Debug, Serialize)]
pub struct ExportReport {
    pub status: ExportStatus,
    pub native_workflow_status: &'static str,
    pub scope: &'static str,
    pub message: &'static str,
    pub output_bytes: Option<u64>,
    pub source_sha256: Option<String>,
    pub output_sha256: Option<String>,
    pub source_changed: bool,
    pub elapsed_ms: u128,
}

impl ExportReport {
    fn new(status: ExportStatus, message: &'static str, started: Instant) -> Self {
        let native_workflow_status = match status {
            ExportStatus::Exported => "checked",
            ExportStatus::Incomplete => "incomplete",
            ExportStatus::Unavailable | ExportStatus::InvalidInput | ExportStatus::ToolError => "untested",
        };
        Self {
            status, native_workflow_status,
            scope: "fixed_krita_kra_to_png_export", message,
            output_bytes: None, source_sha256: None, output_sha256: None,
            source_changed: false, elapsed_ms: started.elapsed().as_millis(),
        }
    }

    fn attempted(mut self) -> Self {
        self.native_workflow_status = "checked";
        self
    }
}

/// Run Krita's documented CLI export to a unique sibling staging file, then
/// publish by a create-only hard link on the same volume. The host must first
/// authorize and resolve both project-relative paths without linked components.
pub fn export_png(source: &Path, target: &Path) -> ExportReport {
    let started = Instant::now();
    if !has_extension(source, "kra") || !has_extension(target, "png") || target.exists() {
        return ExportReport::new(ExportStatus::InvalidInput, "Choose an unused project-local PNG target and a KRA source.", started);
    }
    let Ok(source_meta) = fs::symlink_metadata(source) else {
        return ExportReport::new(ExportStatus::InvalidInput, "The KRA source is unavailable.", started);
    };
    if !source_meta.is_file() || source_meta.file_type().is_symlink()
        || is_reparse_point(&source_meta) || source_meta.len() > MAX_SOURCE_BYTES {
        return ExportReport::new(ExportStatus::InvalidInput, "The KRA source is not a bounded regular file.", started);
    }
    let Some(executable) = installed_krita() else {
        return ExportReport::new(ExportStatus::Unavailable, "A standard local Krita installation was not found.", started);
    };
    let Some(source_digest) = sha256_file(source, MAX_SOURCE_BYTES) else {
        return ExportReport::new(ExportStatus::InvalidInput, "The KRA source could not be read safely.", started);
    };
    let Some(stage) = reserve_sibling(target) else {
        return ExportReport::new(ExportStatus::ToolError, "A temporary export file could not be reserved.", started);
    };
    // Krita needs to create the output itself. The name is unique and remains
    // inside the same verified project directory as the final target.
    if fs::remove_file(&stage).is_err() {
        cleanup(&stage);
        return ExportReport::new(ExportStatus::ToolError, "A temporary export file could not be prepared.", started);
    }
    let child = Command::new(executable)
        .arg(source).arg("--export").arg("--export-filename").arg(&stage)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    let Ok(mut child) = child else {
        cleanup(&stage);
        return ExportReport::new(ExportStatus::ToolError, "Krita could not be started.", started);
    };
    let process_started = Instant::now();
    let mut exceeded = false;
    let exit = loop {
        match child.try_wait() {
            Ok(Some(exit)) => break Some(exit),
            Ok(None) => {
                if stage_over_limit(&stage) { exceeded = true; }
                if exceeded || process_started.elapsed() >= TIMEOUT {
                    let _ = child.kill(); break child.wait().ok();
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(_) => { let _ = child.kill(); break child.wait().ok(); }
        }
    };
    exceeded |= stage_over_limit(&stage);
    if exceeded {
        cleanup(&stage);
        return ExportReport::new(ExportStatus::Incomplete, "Krita exceeded the export size limit.", started);
    }
    if process_started.elapsed() >= TIMEOUT {
        cleanup(&stage);
        return ExportReport::new(ExportStatus::Incomplete, "Krita exceeded the export time limit.", started);
    }
    if exit.is_none_or(|status| !status.success()) {
        cleanup(&stage);
        return ExportReport::new(ExportStatus::ToolError, "Krita did not complete the export.", started).attempted();
    }
    let output_bytes = match valid_png_stage(&stage) {
        Some(bytes) => bytes,
        None => {
            cleanup(&stage);
            return ExportReport::new(ExportStatus::ToolError, "Krita did not produce a bounded PNG file.", started).attempted();
        }
    };
    let Some(source_after) = sha256_file(source, MAX_SOURCE_BYTES) else {
        cleanup(&stage);
        return ExportReport::new(ExportStatus::ToolError, "The KRA source could not be reverified.", started).attempted();
    };
    if source_after != source_digest {
        cleanup(&stage);
        let mut report = ExportReport::new(ExportStatus::ToolError, "The KRA source changed during export.", started).attempted();
        report.source_changed = true;
        return report;
    }
    let Some(output_digest) = sha256_file(&stage, MAX_EXPORT_BYTES) else {
        cleanup(&stage);
        return ExportReport::new(ExportStatus::ToolError, "The PNG output could not be verified.", started).attempted();
    };
    if target.exists() || fs::hard_link(&stage, target).is_err() {
        cleanup(&stage);
        return ExportReport::new(ExportStatus::ToolError, "The target became unavailable before publication.", started).attempted();
    }
    cleanup(&stage);
    let mut report = ExportReport::new(ExportStatus::Exported, "Krita exported a PNG file.", started);
    report.output_bytes = Some(output_bytes);
    report.source_sha256 = Some(source_digest);
    report.output_sha256 = Some(output_digest);
    report
}

fn installed_krita() -> Option<PathBuf> {
    #[cfg(not(windows))]
    { None }
    #[cfg(windows)]
    {
        for base in [env::var_os("ProgramFiles"), env::var_os("ProgramFiles(x86)")].into_iter().flatten() {
            for folder in ["Krita (x64)", "Krita"] {
                let candidate = Path::new(&base).join(folder).join("bin").join("krita.exe");
                if safe_executable(&candidate) { return Some(candidate); }
            }
        }
        None
    }
}

#[cfg(windows)]
fn safe_executable(path: &Path) -> bool {
    let mut current = path;
    loop {
        let Ok(meta) = fs::symlink_metadata(current) else { return false; };
        if meta.file_type().is_symlink() || is_reparse_point(&meta) { return false; }
        if current == path && !meta.is_file() { return false; }
        let Some(parent) = current.parent() else { break; };
        if parent == current { break; }
        current = parent;
    }
    true
}

fn reserve_sibling(target: &Path) -> Option<PathBuf> {
    let parent = target.parent()?;
    for _ in 0..8 {
        let suffix = random_suffix()?;
        let stage = parent.join(format!(".relay-krita-{suffix}.png"));
        if OpenOptions::new().write(true).create_new(true).open(&stage).is_ok() {
            return Some(stage);
        }
    }
    None
}

#[cfg(windows)]
fn random_suffix() -> Option<String> {
    use windows_sys::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};
    let mut bytes = [0u8; 16];
    let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), bytes.as_mut_ptr(), bytes.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status != 0 { return None; }
    Some(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(not(windows))]
fn random_suffix() -> Option<String> { None }

fn stage_over_limit(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(meta) => !meta.is_file() || meta.file_type().is_symlink()
            || is_reparse_point(&meta) || meta.len() > MAX_EXPORT_BYTES,
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

fn valid_png_stage(path: &Path) -> Option<u64> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.file_type().is_symlink() || is_reparse_point(&meta)
        || meta.len() < 33 || meta.len() > MAX_EXPORT_BYTES { return None; }
    let mut header = [0u8; 24];
    File::open(path).ok()?.read_exact(&mut header).ok()?;
    let width = u32::from_be_bytes(header[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(header[20..24].try_into().ok()?);
    (header[..8] == PNG_SIGNATURE[..] && header[8..12] == [0, 0, 0, 13]
        && header[12..16] == *b"IHDR" && width > 0 && height > 0).then_some(meta.len())
}

fn sha256_file(path: &Path, max_bytes: u64) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut chunk).ok()?;
        if read == 0 { break; }
        total = total.checked_add(read as u64)?;
        if total > max_bytes { return None; }
        hasher.update(&chunk[..read]);
    }
    Some(hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
}

fn cleanup(path: &Path) { let _ = fs::remove_file(path); }

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension().and_then(|part| part.to_str()).is_some_and(|part| part.eq_ignore_ascii_case(ext))
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool { false }

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_wrong_formats_without_launch() {
        let report = export_png(Path::new("source.psd"), Path::new("target.png"));
        assert_eq!(report.status, ExportStatus::InvalidInput);
        assert_eq!(report.native_workflow_status, "untested");
    }

    #[test]
    fn png_header_and_digest_checks_do_not_imply_content_validation() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("relay-krita-png-{}-{nonce}.png", std::process::id()));
        fs::write(&path, b"not a png").unwrap();
        assert_eq!(valid_png_stage(&path), None);
        let mut header = Vec::from(PNG_SIGNATURE.as_slice());
        header.extend_from_slice(&[0, 0, 0, 13]);
        header.extend_from_slice(b"IHDR");
        header.extend_from_slice(&1u32.to_be_bytes());
        header.extend_from_slice(&1u32.to_be_bytes());
        header.extend_from_slice(&[0; 9]);
        fs::write(&path, &header).unwrap();
        assert_eq!(valid_png_stage(&path), Some(33));
        assert_eq!(sha256_file(&path, 32), None);
        assert_eq!(sha256_file(&path, 33).unwrap().len(), 64);
        fs::remove_file(path).unwrap();
    }
}
