use relay_contracts::CommandRequest;
use relay_core::service::{ExtensionError, RelayCore};
use relay_verse::{AssertionSpec, CaptureContext, CaptureSourceKind};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::Path;

const MAX_CAPTURE_BYTES: u64 = 32_768;

/// Analyze bounded caller text or a registered project's local log file.
/// Neither route establishes a verified live UEFN session.
pub fn execute(
    core: &RelayCore,
    request: &CommandRequest,
) -> Option<Result<Value, ExtensionError>> {
    let from_file = match request.command.as_str() {
        "runtime.capture.analyze" => false,
        "runtime.capture.file.analyze" => true,
        _ => return None,
    };
    Some((|| {
        let project_id = request.arguments["project_id"].as_str().unwrap_or_default();
        core.require_registered_project(project_id)?;
        let (context, capture_text, file_evidence) = if from_file {
            let root = core.trusted_project_root(project_id)?;
            let relative = request.arguments["relative_path"]
                .as_str()
                .unwrap_or_default();
            let bytes = read_project_capture(&root, relative)?;
            let digest = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let text = String::from_utf8(bytes).map_err(|_| {
                ExtensionError::new("CAPTURE_FILE_INVALID", "capture file must be UTF-8 text")
            })?;
            let context = CaptureContext {
                session_id: request.arguments["session_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                source_kind: CaptureSourceKind::ImportedLog,
                source_version: "project-file-v1".to_string(),
                capture_ref: format!("CAPFILE-{digest}"),
            };
            let evidence = json!({
                "acquisition_kind": "project_local_file",
                "capture_sha256": digest,
                "capture_bytes": text.len()
            });
            (context, text, Some(evidence))
        } else {
            let source_kind = match request.arguments["source_kind"].as_str() {
                Some("uefn_log") => CaptureSourceKind::UefnLog,
                Some("imported_log") => CaptureSourceKind::ImportedLog,
                Some("synthetic_fixture") => CaptureSourceKind::SyntheticFixture,
                _ => {
                    return Err(ExtensionError::new(
                        "CAPTURE_INVALID",
                        "capture source is invalid",
                    ));
                }
            };
            let context = CaptureContext {
                session_id: request.arguments["session_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                source_kind,
                source_version: request.arguments["source_version"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                capture_ref: request.arguments["capture_ref"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            };
            let text = request.arguments["capture_text"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            (context, text, None)
        };
        let specs: Vec<AssertionSpec> = serde_json::from_value(
            request
                .arguments
                .get("assertions")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|_| {
            ExtensionError::new(
                "ASSERTION_SPEC_INVALID",
                "assertion specification is invalid",
            )
        })?;
        let report = relay_verse::normalize_capture(&context, &capture_text)
            .map_err(|_| ExtensionError::new("CAPTURE_INVALID", "capture could not be analyzed"))?;
        let evaluation = relay_verse::evaluate_assertions(&report, &specs);
        let mut result = json!({
            "schema_version": 1,
            "session_id": context.session_id,
            "source_kind": context.source_kind,
            "source_version": context.source_version,
            "capture_ref": context.capture_ref,
            "counts": report.counts,
            "quality": report.quality,
            "assertions": evaluation.results,
            "omitted_assertions": evaluation.omitted_specs,
            "live_uefn_status": "untested"
        });
        if let Some(evidence) = file_evidence {
            result["acquisition_kind"] = evidence["acquisition_kind"].clone();
            result["capture_sha256"] = evidence["capture_sha256"].clone();
            result["capture_bytes"] = evidence["capture_bytes"].clone();
        }
        Ok(result)
    })())
}

fn read_project_capture(root: &Path, relative: &str) -> Result<Vec<u8>, ExtensionError> {
    let invalid = || {
        ExtensionError::new(
            "CAPTURE_PATH_INVALID",
            "project-relative capture path is invalid",
        )
    };
    if !valid_relative_capture(relative) {
        return Err(invalid());
    }
    let parts: Vec<&str> = relative.split('/').collect();
    let mut path = root.to_path_buf();
    for (index, part) in parts.iter().enumerate() {
        path.push(part);
        let metadata = fs::symlink_metadata(&path).map_err(|_| {
            ExtensionError::new("CAPTURE_FILE_UNAVAILABLE", "capture file is unavailable")
        })?;
        if metadata.file_type().is_symlink()
            || is_reparse_point(&metadata)
            || (index + 1 < parts.len() && !metadata.is_dir())
            || (index + 1 == parts.len() && !metadata.is_file())
        {
            return Err(invalid());
        }
    }
    let canonical = fs::canonicalize(path).map_err(|_| invalid())?;
    if !canonical.starts_with(root) {
        return Err(invalid());
    }
    let mut file = open_capture(&canonical).map_err(|_| {
        ExtensionError::new("CAPTURE_FILE_UNAVAILABLE", "capture file is unavailable")
    })?;
    if !opened_within_root(&file, root) {
        return Err(invalid());
    }
    let before = file.metadata().map_err(|_| {
        ExtensionError::new("CAPTURE_FILE_UNAVAILABLE", "capture file is unavailable")
    })?;
    if !before.is_file() {
        return Err(invalid());
    }
    if before.len() > MAX_CAPTURE_BYTES {
        return Err(ExtensionError::new(
            "CAPTURE_FILE_TOO_LARGE",
            "capture file exceeds 32 KiB",
        ));
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    file.by_ref()
        .take(MAX_CAPTURE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            ExtensionError::new("CAPTURE_FILE_UNAVAILABLE", "capture file could not be read")
        })?;
    if bytes.len() as u64 > MAX_CAPTURE_BYTES {
        return Err(ExtensionError::new(
            "CAPTURE_FILE_TOO_LARGE",
            "capture file exceeds 32 KiB",
        ));
    }
    let after = file.metadata().map_err(|_| {
        ExtensionError::new("CAPTURE_FILE_UNAVAILABLE", "capture file is unavailable")
    })?;
    let unchanged = before.len() == after.len()
        && after.len() == bytes.len() as u64
        && before
            .modified()
            .ok()
            .is_some_and(|time| after.modified().ok() == Some(time));
    if !unchanged {
        return Err(ExtensionError::new(
            "CAPTURE_FILE_CHANGED",
            "capture file changed during read",
        ));
    }
    Ok(bytes)
}

#[cfg(windows)]
fn open_capture(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    // Refuse a concurrent writer while observing this bounded file.
    OpenOptions::new()
        .read(true)
        .share_mode(0x0000_0001)
        .open(path)
}

#[cfg(not(windows))]
fn open_capture(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[cfg(windows)]
fn opened_within_root(file: &File, root: &Path) -> bool {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
    let handle = file.as_raw_handle() as _;
    let length = unsafe { GetFinalPathNameByHandleW(handle, std::ptr::null_mut(), 0, 0) };
    if length == 0 || length > 32_768 {
        return false;
    }
    let mut buffer = vec![0u16; length as usize + 1];
    let written =
        unsafe { GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, 0) };
    if written == 0 || written as usize >= buffer.len() {
        return false;
    }
    Path::new(&OsString::from_wide(&buffer[..written as usize])).starts_with(root)
}

#[cfg(not(windows))]
fn opened_within_root(_file: &File, _root: &Path) -> bool {
    true
}

fn valid_relative_capture(relative: &str) -> bool {
    if relative.is_empty()
        || relative.len() > 512
        || relative.starts_with('/')
        || relative.contains(['\\', ':'])
        || relative.bytes().any(|byte| byte < 32 || byte == 127)
        || !Path::new(relative)
            .extension()
            .and_then(|part| part.to_str())
            .is_some_and(|part| {
                part.eq_ignore_ascii_case("log") || part.eq_ignore_ascii_case("jsonl")
            })
    {
        return false;
    }
    relative.split('/').all(|part| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && !part.ends_with([' ', '.'])
            && !windows_device_name(part)
    })
}

fn windows_device_name(part: &str) -> bool {
    let base = part
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn project_capture_rejects_paths_outside_bounded_log_shape() {
        for path in [
            "../capture.log",
            "/capture.log",
            "C:/capture.log",
            "sub\\capture.log",
            "sub//capture.log",
            "CON.log",
            "capture.txt",
            "sub/../capture.log",
        ] {
            assert!(!valid_relative_capture(path), "{path}");
        }
        assert!(valid_relative_capture("Saved/Logs/session.jsonl"));
    }

    #[test]
    fn project_capture_reads_only_bounded_regular_file() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("relay-verse-file-{}-{nonce}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let root = fs::canonicalize(&dir).unwrap();
        let file = dir.join("capture.log");
        fs::write(&file, b"RELAY_EVENT_V1 {}\n").unwrap();
        assert_eq!(
            read_project_capture(&root, "capture.log")
                .unwrap_or_else(|error| panic!("{}", error.code)),
            b"RELAY_EVENT_V1 {}\n"
        );
        fs::write(&file, vec![b'a'; MAX_CAPTURE_BYTES as usize + 1]).unwrap();
        assert_eq!(
            read_project_capture(&root, "capture.log").unwrap_err().code,
            "CAPTURE_FILE_TOO_LARGE"
        );
        fs::remove_file(&file).unwrap();
        fs::remove_dir(&dir).unwrap();
    }
}
