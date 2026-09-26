use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const RECORD_FORMAT: u32 = 1;
pub const NORMAL_MAX_BYTES: usize = 4 * 1024;
pub const DETAIL_MAX_BYTES: usize = 32 * 1024;
pub const DEFAULT_MAX_FILE_BYTES: u64 = 512 * 1024;
pub const DEFAULT_MAX_FILES: usize = 4;
pub const DEFAULT_SYNC_EVERY: u64 = 16;
pub const DETAIL_MAX_DURATION_MS: u64 = 15 * 60 * 1000;

const CURRENT_FILE: &str = "relay-diagnostics.jsonl";
const META_FILE: &str = "relay-diagnostics.meta.json";

const SENSITIVE_KEYS: &[&str] = &[
    "authorization", "cookie", "credential", "credentials", "password",
    "secret", "token", "api_key", "apikey", "access_key", "refresh_token",
    "path", "project_path", "workspace_path", "home", "userprofile",
];

pub fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Debug, Clone)]
pub struct DiagnosticError {
    pub code: &'static str,
    pub message: String,
}

impl DiagnosticError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl fmt::Display for DiagnosticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for DiagnosticError {}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Critical,
}

impl Severity {
    fn as_key(self) -> &'static str {
        match self {
            Severity::Trace => "trace",
            Severity::Debug => "debug",
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Error => "error",
            Severity::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticRefs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
}

impl Default for DiagnosticRefs {
    fn default() -> Self {
        Self {
            project_id: None,
            job_id: None,
            result_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSource {
    pub kind: String,
    pub name: String,
    pub trust: String,
}

impl DiagnosticSource {
    pub fn trusted_local(name: impl Into<String>) -> Self {
        Self {
            kind: "local".to_string(),
            name: name.into(),
            trust: "trusted".to_string(),
        }
    }

    pub fn untrusted_external(name: impl Into<String>) -> Self {
        Self {
            kind: "external".to_string(),
            name: name.into(),
            trust: "untrusted".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletenessMetadata {
    pub complete: bool,
    pub sampled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<f64>,
    pub lost_before: u64,
}

impl Default for CompletenessMetadata {
    fn default() -> Self {
        Self {
            complete: true,
            sampled: false,
            sample_rate: None,
            lost_before: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureMetadata {
    pub mode: String,
    pub redacted_fields: u32,
    pub truncated_values: u32,
}

impl Default for CaptureMetadata {
    fn default() -> Self {
        Self {
            mode: "normal".to_string(),
            redacted_fields: 0,
            truncated_values: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticEvent {
    pub record_format: u32,
    pub event_id: String,
    pub time_unix_ms: u64,
    pub severity: Severity,
    pub component: String,
    pub message: String,
    pub refs: DiagnosticRefs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    pub source: DiagnosticSource,
    pub completeness: CompletenessMetadata,
    pub capture: CaptureMetadata,
    pub attributes: Map<String, Value>,
}

impl DiagnosticEvent {
    pub fn new(
        event_id: impl Into<String>,
        severity: Severity,
        component: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            record_format: RECORD_FORMAT,
            event_id: event_id.into(),
            time_unix_ms: unix_ms(),
            severity,
            component: component.into(),
            message: message.into(),
            refs: DiagnosticRefs::default(),
            correlation_id: None,
            causation_id: None,
            source: DiagnosticSource::trusted_local("relay"),
            completeness: CompletenessMetadata::default(),
            capture: CaptureMetadata::default(),
            attributes: Map::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticConfig {
    pub directory: PathBuf,
    pub max_file_bytes: u64,
    pub max_files: usize,
    pub sync_every: u64,
}

impl DiagnosticConfig {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_files: DEFAULT_MAX_FILES,
            sync_every: DEFAULT_SYNC_EVERY,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DiagnosticMeta {
    evicted_events: u64,
    evicted_files: u64,
    recovered_partial_bytes: u64,
    last_sync_unix_ms: Option<u64>,
    last_error: Option<String>,
    detail_until_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticHealth {
    pub ok: bool,
    pub current_bytes: u64,
    pub rotated_files: usize,
    pub evicted_events: u64,
    pub evicted_files: u64,
    pub recovered_partial_bytes: u64,
    pub last_sync_unix_ms: Option<u64>,
    pub last_error: Option<String>,
    pub detail_active: bool,
    pub detail_until_unix_ms: Option<u64>,
    pub sync_every: u64,
    pub max_file_bytes: u64,
    pub max_files: usize,
}

impl DiagnosticHealth {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            current_bytes: 0,
            rotated_files: 0,
            evicted_events: 0,
            evicted_files: 0,
            recovered_partial_bytes: 0,
            last_sync_unix_ms: None,
            last_error: Some(message.into()),
            detail_active: false,
            detail_until_unix_ms: None,
            sync_every: DEFAULT_SYNC_EVERY,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_files: DEFAULT_MAX_FILES,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AppendOutcome {
    pub bytes_written: usize,
    pub synced: bool,
    pub rotated: bool,
    pub detail_active: bool,
    pub redacted_fields: u32,
    pub truncated_values: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticAggregate {
    pub total_events: u64,
    pub invalid_lines: u64,
    pub incomplete_events: u64,
    pub sampled_events: u64,
    pub detail_events: u64,
    pub untrusted_source_events: u64,
    pub by_severity: BTreeMap<String, u64>,
    pub by_component: BTreeMap<String, u64>,
    pub by_event_id: BTreeMap<String, u64>,
    pub retention_evicted_events: u64,
    pub retention_evicted_files: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupportSummary {
    pub health: DiagnosticHealth,
    pub aggregate: DiagnosticAggregate,
    pub includes_raw_history: bool,
    pub redaction_policy: &'static str,
}

pub struct JsonlDiagnostics {
    config: DiagnosticConfig,
    current_path: PathBuf,
    meta_path: PathBuf,
    file: Option<File>,
    current_bytes: u64,
    writes_since_sync: u64,
    meta: DiagnosticMeta,
}

impl JsonlDiagnostics {
    pub fn open(config: DiagnosticConfig) -> Result<Self, DiagnosticError> {
        if config.max_files == 0 {
            return Err(DiagnosticError::new(
                "DIAGNOSTICS_CONFIG_INVALID",
                "max_files must be at least 1",
            ));
        }
        if config.sync_every == 0 {
            return Err(DiagnosticError::new(
                "DIAGNOSTICS_CONFIG_INVALID",
                "sync_every must be at least 1",
            ));
        }
        fs::create_dir_all(&config.directory).map_err(|error| {
            DiagnosticError::new(
                "DIAGNOSTICS_OPEN_FAILED",
                format!("create diagnostic directory: {error}"),
            )
        })?;
        let current_path = config.directory.join(CURRENT_FILE);
        let meta_path = config.directory.join(META_FILE);
        let (mut meta, meta_error) = load_meta(&meta_path);
        if let Some(error) = meta_error {
            meta.last_error = Some(error);
        }
        let recovered = recover_partial_tail(&current_path)?;
        if recovered > 0 {
            meta.recovered_partial_bytes =
                meta.recovered_partial_bytes.saturating_add(recovered);
        }
        let current_bytes = fs::metadata(&current_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let file = open_append_file(&current_path)?;
        let diagnostics = Self {
            config,
            current_path,
            meta_path,
            file: Some(file),
            current_bytes,
            writes_since_sync: 0,
            meta,
        };
        if recovered > 0 {
            let _ = diagnostics.persist_meta();
        }
        Ok(diagnostics)
    }

    pub fn enable_detail(&mut self, duration_ms: u64) -> Result<u64, DiagnosticError> {
        let bounded = duration_ms.min(DETAIL_MAX_DURATION_MS);
        let until = unix_ms().saturating_add(bounded);
        self.meta.detail_until_unix_ms = Some(until);
        self.persist_meta()?;
        Ok(until)
    }

    pub fn disable_detail(&mut self) -> Result<(), DiagnosticError> {
        self.meta.detail_until_unix_ms = None;
        self.persist_meta()
    }

    pub fn append(&mut self, mut event: DiagnosticEvent) -> Result<AppendOutcome, DiagnosticError> {
        let detail_active = self.detail_active();
        sanitize_event(&mut event, detail_active);
        let max_bytes = if detail_active {
            DETAIL_MAX_BYTES
        } else {
            NORMAL_MAX_BYTES
        };

        let mut encoded = encode_event(&event)?;
        if encoded.len() > max_bytes {
            let original_bytes = encoded.len();
            event.attributes = Map::from_iter([(
                "_relay_truncated_event".to_string(),
                json!({
                    "original_bytes": original_bytes,
                    "reason": "event exceeded capture byte limit"
                }),
            )]);
            event.capture.truncated_values =
                event.capture.truncated_values.saturating_add(1);
            event.completeness.complete = false;
            encoded = encode_event(&event)?;
        }
        if encoded.len() > max_bytes {
            self.record_error(format!(
                "event {} still exceeded {} bytes after truncation",
                event.event_id, max_bytes
            ));
            return Err(DiagnosticError::new(
                "DIAGNOSTICS_EVENT_TOO_LARGE",
                format!("event exceeds {max_bytes} byte capture limit"),
            ));
        }

        let line_bytes = encoded.len() as u64 + 1;
        if line_bytes > self.config.max_file_bytes {
            self.record_error(format!(
                "event {} exceeds configured file bound of {} bytes",
                event.event_id, self.config.max_file_bytes
            ));
            return Err(DiagnosticError::new(
                "DIAGNOSTICS_EVENT_TOO_LARGE",
                "event exceeds configured diagnostic file bound",
            ));
        }

        let mut rotated = false;
        if self.current_bytes > 0
            && self.current_bytes.saturating_add(line_bytes)
                > self.config.max_file_bytes
        {
            self.rotate()?;
            rotated = true;
        }

        let write_result = (|| -> Result<(), std::io::Error> {
            let file = self.file.as_mut().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "diagnostic file handle is unavailable",
                )
            })?;
            file.write_all(&encoded)?;
            file.write_all(b"\n")?;
            file.flush()?;
            Ok(())
        })();
        if let Err(error) = write_result {
            self.record_error(format!("append diagnostic event: {error}"));
            return Err(DiagnosticError::new(
                "DIAGNOSTICS_WRITE_FAILED",
                format!("append diagnostic event: {error}"),
            ));
        }

        let bytes_written = encoded.len() + 1;
        self.current_bytes = self.current_bytes.saturating_add(bytes_written as u64);
        self.writes_since_sync = self.writes_since_sync.saturating_add(1);

        let mut synced = false;
        if self.writes_since_sync >= self.config.sync_every {
            self.sync_data()?;
            synced = true;
        }

        Ok(AppendOutcome {
            bytes_written,
            synced,
            rotated,
            detail_active,
            redacted_fields: event.capture.redacted_fields,
            truncated_values: event.capture.truncated_values,
        })
    }

    pub fn flush(&mut self) -> Result<(), DiagnosticError> {
        self.sync_data()
    }

    pub fn health(&self) -> DiagnosticHealth {
        DiagnosticHealth {
            ok: self.meta.last_error.is_none(),
            current_bytes: self.current_bytes,
            rotated_files: self.rotated_file_count(),
            evicted_events: self.meta.evicted_events,
            evicted_files: self.meta.evicted_files,
            recovered_partial_bytes: self.meta.recovered_partial_bytes,
            last_sync_unix_ms: self.meta.last_sync_unix_ms,
            last_error: self.meta.last_error.clone(),
            detail_active: self.detail_active(),
            detail_until_unix_ms: self.meta.detail_until_unix_ms,
            sync_every: self.config.sync_every,
            max_file_bytes: self.config.max_file_bytes,
            max_files: self.config.max_files,
        }
    }

    pub fn aggregate(&self) -> Result<DiagnosticAggregate, DiagnosticError> {
        aggregate_paths(
            &self.log_paths_oldest_first(),
            self.meta.evicted_events,
            self.meta.evicted_files,
        )
    }

    pub fn support_summary(&self) -> Result<SupportSummary, DiagnosticError> {
        Ok(SupportSummary {
            health: self.health(),
            aggregate: self.aggregate()?,
            includes_raw_history: false,
            redaction_policy: "deterministic-sensitive-key-and-path-redaction",
        })
    }

    fn detail_active(&self) -> bool {
        self.meta
            .detail_until_unix_ms
            .map(|until| unix_ms() <= until)
            .unwrap_or(false)
    }

    fn sync_data(&mut self) -> Result<(), DiagnosticError> {
        let result = match self.file.as_mut() {
            Some(file) => file.sync_data(),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "diagnostic file handle is unavailable",
            )),
        };
        if let Err(error) = result {
            self.record_error(format!("sync diagnostic file: {error}"));
            return Err(DiagnosticError::new(
                "DIAGNOSTICS_SYNC_FAILED",
                format!("sync diagnostic file: {error}"),
            ));
        }
        self.writes_since_sync = 0;
        self.meta.last_sync_unix_ms = Some(unix_ms());
        self.persist_meta()
    }

    fn rotate(&mut self) -> Result<(), DiagnosticError> {
        if let Some(file) = self.file.take() {
            if let Err(error) = file.sync_data() {
                self.record_error(format!("sync before rotation: {error}"));
                return Err(DiagnosticError::new(
                    "DIAGNOSTICS_ROTATE_FAILED",
                    format!("sync before rotation: {error}"),
                ));
            }
            drop(file);
        }

        let rotated_slots = self.config.max_files.saturating_sub(1);
        if rotated_slots == 0 {
            if self.current_path.exists() {
                self.meta.evicted_events = self
                    .meta
                    .evicted_events
                    .saturating_add(count_valid_lines(&self.current_path));
                self.meta.evicted_files = self.meta.evicted_files.saturating_add(1);
                fs::remove_file(&self.current_path).map_err(|error| {
                    DiagnosticError::new(
                        "DIAGNOSTICS_ROTATE_FAILED",
                        format!("remove single retained log: {error}"),
                    )
                })?;
            }
        } else {
            let oldest = self.rotated_path(rotated_slots);
            if oldest.exists() {
                self.meta.evicted_events = self
                    .meta
                    .evicted_events
                    .saturating_add(count_valid_lines(&oldest));
                self.meta.evicted_files = self.meta.evicted_files.saturating_add(1);
                fs::remove_file(&oldest).map_err(|error| {
                    DiagnosticError::new(
                        "DIAGNOSTICS_ROTATE_FAILED",
                        format!("remove oldest rotated log: {error}"),
                    )
                })?;
            }

            for slot in (1..rotated_slots).rev() {
                let from = self.rotated_path(slot);
                let to = self.rotated_path(slot + 1);
                if from.exists() {
                    fs::rename(&from, &to).map_err(|error| {
                        DiagnosticError::new(
                            "DIAGNOSTICS_ROTATE_FAILED",
                            format!("shift rotated log: {error}"),
                        )
                    })?;
                }
            }

            if self.current_path.exists() {
                fs::rename(&self.current_path, self.rotated_path(1)).map_err(|error| {
                    DiagnosticError::new(
                        "DIAGNOSTICS_ROTATE_FAILED",
                        format!("rotate current diagnostic log: {error}"),
                    )
                })?;
            }
        }

        self.file = Some(open_append_file(&self.current_path)?);
        self.current_bytes = 0;
        self.writes_since_sync = 0;
        self.meta.last_sync_unix_ms = Some(unix_ms());
        self.persist_meta()
    }

    fn rotated_path(&self, slot: usize) -> PathBuf {
        self.config
            .directory
            .join(format!("relay-diagnostics.{slot}.jsonl"))
    }

    fn rotated_file_count(&self) -> usize {
        (1..self.config.max_files)
            .filter(|slot| self.rotated_path(*slot).exists())
            .count()
    }

    fn log_paths_oldest_first(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for slot in (1..self.config.max_files).rev() {
            let path = self.rotated_path(slot);
            if path.exists() {
                paths.push(path);
            }
        }
        if self.current_path.exists() {
            paths.push(self.current_path.clone());
        }
        paths
    }

    fn record_error(&mut self, message: String) {
        self.meta.last_error = Some(message);
        let _ = self.persist_meta();
    }

    fn persist_meta(&self) -> Result<(), DiagnosticError> {
        let bytes = serde_json::to_vec_pretty(&self.meta).map_err(|error| {
            DiagnosticError::new(
                "DIAGNOSTICS_META_FAILED",
                format!("serialize diagnostic metadata: {error}"),
            )
        })?;
        fs::write(&self.meta_path, bytes).map_err(|error| {
            DiagnosticError::new(
                "DIAGNOSTICS_META_FAILED",
                format!("write diagnostic metadata: {error}"),
            )
        })
    }
}

fn open_append_file(path: &Path) -> Result<File, DiagnosticError> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(path)
        .map_err(|error| {
            DiagnosticError::new(
                "DIAGNOSTICS_OPEN_FAILED",
                format!("open diagnostic log: {error}"),
            )
        })
}

fn load_meta(path: &Path) -> (DiagnosticMeta, Option<String>) {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<DiagnosticMeta>(&bytes) {
            Ok(meta) => (meta, None),
            Err(error) => (
                DiagnosticMeta::default(),
                Some(format!(
                    "diagnostic metadata is unreadable; completeness counters may be incomplete: {error}"
                )),
            ),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            (DiagnosticMeta::default(), None)
        }
        Err(error) => (
            DiagnosticMeta::default(),
            Some(format!(
                "diagnostic metadata could not be read; completeness counters may be incomplete: {error}"
            )),
        ),
    }
}

fn recover_partial_tail(path: &Path) -> Result<u64, DiagnosticError> {
    if !path.exists() {
        return Ok(0);
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|error| {
            DiagnosticError::new(
                "DIAGNOSTICS_RECOVERY_FAILED",
                format!("read diagnostic tail: {error}"),
            )
        })?;
    if bytes.is_empty() || bytes.last() == Some(&b'\n') {
        return Ok(0);
    }

    let keep = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let removed = bytes.len().saturating_sub(keep) as u64;
    OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|file| file.set_len(keep as u64))
        .map_err(|error| {
            DiagnosticError::new(
                "DIAGNOSTICS_RECOVERY_FAILED",
                format!("truncate partial diagnostic tail: {error}"),
            )
        })?;
    Ok(removed)
}

fn encode_event(event: &DiagnosticEvent) -> Result<Vec<u8>, DiagnosticError> {
    serde_json::to_vec(event).map_err(|error| {
        DiagnosticError::new(
            "DIAGNOSTICS_SERIALIZE_FAILED",
            format!("serialize diagnostic event: {error}"),
        )
    })
}

fn bounded_string(value: &str, limit: usize, truncated: &mut u32) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    *truncated = truncated.saturating_add(1);
    value.chars().take(limit).collect::<String>() + "…"
}

fn sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace('-', "_");
    SENSITIVE_KEYS.iter().any(|candidate| {
        normalized == *candidate
            || normalized.ends_with(&format!("_{candidate}"))
            || normalized.contains(&format!("{candidate}_"))
    })
}

fn looks_like_private_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with(r"c:\")
        || lower.starts_with(r"\\")
        || lower.starts_with("/home/")
        || lower.starts_with("file:///")
        || lower.contains(r"\users\")
        || lower.contains("/users/")
}

fn sanitize_value(
    value: &Value,
    detail: bool,
    redacted: &mut u32,
    truncated: &mut u32,
) -> Value {
    let string_limit = if detail { 4096 } else { 256 };
    let array_limit = if detail { 128 } else { 16 };
    let object_limit = if detail { 128 } else { 32 };

    match value {
        Value::String(text) => {
            if looks_like_private_path(text) {
                *redacted = redacted.saturating_add(1);
                Value::String("[REDACTED_PATH]".to_string())
            } else {
                Value::String(bounded_string(text, string_limit, truncated))
            }
        }
        Value::Array(items) => {
            if items.len() > array_limit {
                *truncated = truncated.saturating_add(1);
            }
            Value::Array(
                items
                    .iter()
                    .take(array_limit)
                    .map(|item| sanitize_value(item, detail, redacted, truncated))
                    .collect(),
            )
        }
        Value::Object(object) => {
            let mut output = Map::new();
            if object.len() > object_limit {
                *truncated = truncated.saturating_add(1);
            }
            for (index, (key, item)) in object.iter().enumerate() {
                if index >= object_limit {
                    break;
                }
                if sensitive_key(key) {
                    *redacted = redacted.saturating_add(1);
                    output.insert(key.clone(), Value::String("[REDACTED]".to_string()));
                } else {
                    output.insert(
                        key.clone(),
                        sanitize_value(item, detail, redacted, truncated),
                    );
                }
            }
            Value::Object(output)
        }
        other => other.clone(),
    }
}

fn sanitize_event(event: &mut DiagnosticEvent, detail: bool) {
    let mut redacted = 0u32;
    let mut truncated = 0u32;
    event.event_id = bounded_string(&event.event_id, 128, &mut truncated);
    event.component = bounded_string(&event.component, 128, &mut truncated);
    if looks_like_private_path(&event.message) {
        event.message = "[REDACTED_PATH]".to_string();
        redacted = redacted.saturating_add(1);
    } else {
        event.message = bounded_string(
            &event.message,
            if detail { 4096 } else { 512 },
            &mut truncated,
        );
    }
    event.source.kind = bounded_string(&event.source.kind, 64, &mut truncated);
    if looks_like_private_path(&event.source.name) {
        event.source.name = "[REDACTED_PATH]".to_string();
        redacted = redacted.saturating_add(1);
    } else {
        event.source.name =
            bounded_string(&event.source.name, 128, &mut truncated);
    }
    event.source.trust = bounded_string(&event.source.trust, 32, &mut truncated);

    for reference in [
        &mut event.refs.project_id,
        &mut event.refs.job_id,
        &mut event.refs.result_id,
        &mut event.correlation_id,
        &mut event.causation_id,
    ] {
        if let Some(value) = reference {
            if looks_like_private_path(value) {
                *value = "[REDACTED_PATH]".to_string();
                redacted = redacted.saturating_add(1);
            } else {
                *value = bounded_string(value, 128, &mut truncated);
            }
        }
    }

    let value = sanitize_value(
        &Value::Object(event.attributes.clone()),
        detail,
        &mut redacted,
        &mut truncated,
    );
    event.attributes = value.as_object().cloned().unwrap_or_default();
    event.capture = CaptureMetadata {
        mode: if detail { "detail" } else { "normal" }.to_string(),
        redacted_fields: redacted,
        truncated_values: truncated,
    };
    if truncated > 0 {
        event.completeness.complete = false;
    }
}

fn count_valid_lines(path: &Path) -> u64 {
    let Ok(file) = File::open(path) else {
        return 0;
    };
    BufReader::new(file)
        .lines()
        .filter_map(Result::ok)
        .filter(|line| serde_json::from_str::<DiagnosticEvent>(line).is_ok())
        .count() as u64
}

fn aggregate_paths(
    paths: &[PathBuf],
    evicted_events: u64,
    evicted_files: u64,
) -> Result<DiagnosticAggregate, DiagnosticError> {
    let mut total_events = 0u64;
    let mut invalid_lines = 0u64;
    let mut incomplete_events = 0u64;
    let mut sampled_events = 0u64;
    let mut detail_events = 0u64;
    let mut untrusted_source_events = 0u64;
    let mut by_severity = BTreeMap::new();
    let mut by_component = BTreeMap::new();
    let mut by_event_id = BTreeMap::new();

    for path in paths {
        let file = File::open(path).map_err(|error| {
            DiagnosticError::new(
                "DIAGNOSTICS_READ_FAILED",
                format!("open diagnostic log for aggregation: {error}"),
            )
        })?;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| {
                DiagnosticError::new(
                    "DIAGNOSTICS_READ_FAILED",
                    format!("read diagnostic line: {error}"),
                )
            })?;
            let event: DiagnosticEvent = match serde_json::from_str(&line) {
                Ok(event) => event,
                Err(_) => {
                    invalid_lines = invalid_lines.saturating_add(1);
                    continue;
                }
            };
            total_events = total_events.saturating_add(1);
            *by_severity
                .entry(event.severity.as_key().to_string())
                .or_insert(0) += 1;
            *by_component.entry(event.component.clone()).or_insert(0) += 1;
            *by_event_id.entry(event.event_id.clone()).or_insert(0) += 1;
            if !event.completeness.complete {
                incomplete_events = incomplete_events.saturating_add(1);
            }
            if event.completeness.sampled {
                sampled_events = sampled_events.saturating_add(1);
            }
            if event.capture.mode == "detail" {
                detail_events = detail_events.saturating_add(1);
            }
            if event.source.trust != "trusted" {
                untrusted_source_events = untrusted_source_events.saturating_add(1);
            }
        }
    }

    Ok(DiagnosticAggregate {
        total_events,
        invalid_lines,
        incomplete_events,
        sampled_events,
        detail_events,
        untrusted_source_events,
        by_severity,
        by_component,
        by_event_id,
        retention_evicted_events: evicted_events,
        retention_evicted_files: evicted_files,
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "relay-diagnostics-{label}-{}-{suffix}",
            std::process::id()
        ))
    }

    fn sample_event(index: usize) -> DiagnosticEvent {
        let mut event = DiagnosticEvent::new(
            "relay.test.event",
            Severity::Info,
            "fixture.component",
            format!("fixture event {index}"),
        );
        event.correlation_id = Some(format!("CORR-{index}"));
        event.attributes.insert("sequence".to_string(), json!(index));
        event
    }

    #[test]
    fn sensitive_fields_and_private_paths_are_redacted() {
        let dir = temp_dir("redaction");
        let mut diagnostics =
            JsonlDiagnostics::open(DiagnosticConfig::new(&dir)).unwrap();
        let mut event = sample_event(1);
        event
            .attributes
            .insert("auth_token".to_string(), json!("super-secret"));
        event.attributes.insert(
            "note".to_string(),
            json!(r"C:\Fixture\private\project"),
        );
        diagnostics.append(event).unwrap();
        diagnostics.flush().unwrap();

        let raw = fs::read_to_string(dir.join(CURRENT_FILE)).unwrap();
        assert!(!raw.contains("super-secret"));
        assert!(!raw.contains("someone"));
        assert!(raw.contains("[REDACTED]"));
        assert!(raw.contains("[REDACTED_PATH]"));
        drop(diagnostics);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rotation_is_bounded_and_reports_eviction() {
        let dir = temp_dir("rotation");
        let mut config = DiagnosticConfig::new(&dir);
        config.max_file_bytes = 700;
        config.max_files = 2;
        config.sync_every = 2;
        let mut diagnostics = JsonlDiagnostics::open(config).unwrap();

        for index in 0..40 {
            diagnostics.append(sample_event(index)).unwrap();
        }
        diagnostics.flush().unwrap();

        let health = diagnostics.health();
        assert!(health.rotated_files <= 1);
        assert!(health.evicted_files > 0);
        assert!(health.evicted_events > 0);
        assert!(health.current_bytes <= 700);
        let aggregate = diagnostics.aggregate().unwrap();
        assert_eq!(aggregate.retention_evicted_events, health.evicted_events);
        drop(diagnostics);
        fs::remove_dir_all(dir).unwrap();
    }


    #[test]
    fn partial_tail_is_removed_on_reopen() {
        let dir = temp_dir("tail");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(CURRENT_FILE);
        let mut first = sample_event(1);
        first.capture.mode = "normal".to_string();
        let valid = serde_json::to_string(&first).unwrap();
        fs::write(&path, format!("{valid}\n{{\"partial\":")).unwrap();

        let diagnostics =
            JsonlDiagnostics::open(DiagnosticConfig::new(&dir)).unwrap();
        let health = diagnostics.health();
        assert!(health.recovered_partial_bytes > 0);
        let raw = fs::read_to_string(&path).unwrap();
        assert_eq!(raw, format!("{valid}\n"));
        drop(diagnostics);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn detail_mode_is_explicit_capped_and_expires_in_health() {
        let dir = temp_dir("detail");
        let mut diagnostics =
            JsonlDiagnostics::open(DiagnosticConfig::new(&dir)).unwrap();
        let until = diagnostics
            .enable_detail(DETAIL_MAX_DURATION_MS + 60_000)
            .unwrap();
        assert!(until <= unix_ms().saturating_add(DETAIL_MAX_DURATION_MS + 100));
        assert!(diagnostics.health().detail_active);

        let mut event = sample_event(2);
        event.attributes.insert(
            "large_detail".to_string(),
            json!("x".repeat(3000)),
        );
        let outcome = diagnostics.append(event).unwrap();
        assert!(outcome.detail_active);
        assert_eq!(outcome.truncated_values, 0);

        diagnostics.disable_detail().unwrap();
        assert!(!diagnostics.health().detail_active);
        drop(diagnostics);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn support_summary_contains_counts_not_raw_history() {
        let dir = temp_dir("support");
        let mut diagnostics =
            JsonlDiagnostics::open(DiagnosticConfig::new(&dir)).unwrap();
        let mut event = sample_event(3);
        event.source = DiagnosticSource::untrusted_external("synthetic-tool");
        event.completeness.sampled = true;
        event.completeness.sample_rate = Some(0.5);
        event
            .attributes
            .insert("password".to_string(), json!("never-export-this"));
        diagnostics.append(event).unwrap();
        diagnostics.flush().unwrap();

        let summary = diagnostics.support_summary().unwrap();
        assert_eq!(summary.aggregate.total_events, 1);
        assert_eq!(summary.aggregate.sampled_events, 1);
        assert_eq!(summary.aggregate.untrusted_source_events, 1);
        assert!(!summary.includes_raw_history);

        let serialized = serde_json::to_string(&summary).unwrap();
        assert!(!serialized.contains("never-export-this"));
        assert!(!serialized.contains("fixture event 3"));
        drop(diagnostics);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn normal_mode_truncates_large_detail_without_dropping_event() {
        let dir = temp_dir("normal-truncate");
        let mut diagnostics =
            JsonlDiagnostics::open(DiagnosticConfig::new(&dir)).unwrap();
        let mut event = sample_event(4);
        event.attributes.insert(
            "detail_blob".to_string(),
            json!("z".repeat(10_000)),
        );
        let outcome = diagnostics.append(event).unwrap();
        assert!(outcome.truncated_values > 0);
        let aggregate = diagnostics.aggregate().unwrap();
        assert_eq!(aggregate.total_events, 1);
        assert_eq!(aggregate.incomplete_events, 1);
        drop(diagnostics);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupt_metadata_degrades_health_instead_of_going_false_green() {
        let dir = temp_dir("corrupt-meta");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(META_FILE), b"not-json").unwrap();

        let diagnostics =
            JsonlDiagnostics::open(DiagnosticConfig::new(&dir)).unwrap();
        let health = diagnostics.health();
        assert!(!health.ok);
        assert!(health
            .last_error
            .as_deref()
            .unwrap_or_default()
            .contains("completeness counters may be incomplete"));

        drop(diagnostics);
        fs::remove_dir_all(dir).unwrap();
    }
}
