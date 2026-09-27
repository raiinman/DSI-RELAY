//! Local, bounded normalization of structured Verse log events.
//!
//! This crate parses caller-supplied capture text. It does not attach to UEFN,
//! authenticate a play session, or claim that an assertion passed in live play.

use serde::{Deserialize, Serialize};

pub const EVENT_PREFIX: &str = "RELAY_EVENT_V1 ";
pub const EVENT_SCHEMA_VERSION: u32 = 1;
pub const MAX_CAPTURE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_LINES: usize = 8192;
pub const MAX_LINE_BYTES: usize = 4096;
pub const MAX_EVENTS: usize = 2048;
pub const MAX_ASSERTIONS: usize = 128;
const MAX_SESSION_MS: u64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSourceKind {
    /// A caller says this came from a UEFN play-session log. This crate cannot verify that claim.
    UefnLog,
    ImportedLog,
    SyntheticFixture,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureContext {
    pub session_id: String,
    pub source_kind: CaptureSourceKind,
    /// Caller-declared source/adapter version, never inferred from the log.
    pub source_version: String,
    /// Opaque capture ID. Do not supply a local path or player identifier.
    pub capture_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureError {
    pub code: &'static str,
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code)
    }
}

impl std::error::Error for CaptureError {}

#[derive(Debug, Clone, Serialize)]
pub struct NormalizedEvent {
    pub session_id: String,
    pub sequence: u64,
    /// Milliseconds relative to the declared play session, not a wall-clock timestamp.
    pub session_time_ms: u64,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numeric_value: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireEvent {
    schema_version: u32,
    session_id: String,
    sequence: u64,
    session_time_ms: u64,
    kind: String,
    #[serde(default)]
    subject_ref: Option<String>,
    #[serde(default)]
    target_ref: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    numeric_value: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CaptureCounts {
    pub lines_seen: usize,
    pub unrelated_lines: usize,
    pub accepted_events: usize,
    pub malformed_events: usize,
    pub wrong_session_events: usize,
    pub duplicate_or_reordered_events: usize,
    pub oversized_lines: usize,
    pub omitted_events: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureQuality {
    pub complete: bool,
    pub observed_session_start: bool,
    pub observed_session_end: bool,
    pub boundary_errors: usize,
    pub sequence_gaps: u64,
    pub time_regressions: usize,
    pub input_truncated: bool,
    /// The declared source is unverified until a trusted adapter ties it to a play session.
    pub source_verified: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureReport {
    pub schema_version: u32,
    pub context: CaptureContext,
    pub counts: CaptureCounts,
    pub quality: CaptureQuality,
    pub events: Vec<NormalizedEvent>,
}

/// Parse a bounded log capture. Non-RELAY lines are counted but never retained.
/// Invalid records are counted without returning raw input or private details.
pub fn normalize_capture(
    context: &CaptureContext,
    capture: &str,
) -> Result<CaptureReport, CaptureError> {
    if !safe_token(&context.session_id, 64)
        || !safe_token(&context.source_version, 64)
        || !safe_token(&context.capture_ref, 96)
    {
        return Err(CaptureError {
            code: "INVALID_CAPTURE_CONTEXT",
        });
    }

    let mut counts = CaptureCounts::default();
    let mut events = Vec::new();
    let mut observed_session_start = false;
    let mut observed_session_end = false;
    let mut boundary_errors = 0usize;
    let mut sequence_gaps = 0u64;
    let mut time_regressions = 0usize;
    let mut last_sequence = 0u64;
    let mut last_time = 0u64;
    let mut input_truncated = !capture.is_empty() && !capture.ends_with('\n');
    let mut bytes_seen = 0usize;

    for line in capture.lines() {
        if counts.lines_seen >= MAX_LINES
            || bytes_seen.saturating_add(line.len()).saturating_add(1) > MAX_CAPTURE_BYTES
        {
            input_truncated = true;
            break;
        }
        counts.lines_seen += 1;
        bytes_seen = bytes_seen.saturating_add(line.len()).saturating_add(1);
        if line.len() > MAX_LINE_BYTES {
            counts.oversized_lines += 1;
            continue;
        }
        let Some(payload) = line.strip_prefix(EVENT_PREFIX) else {
            counts.unrelated_lines += 1;
            continue;
        };
        if events.len() >= MAX_EVENTS {
            counts.omitted_events += 1;
            input_truncated = true;
            continue;
        }
        let Ok(raw) = serde_json::from_str::<WireEvent>(payload) else {
            counts.malformed_events += 1;
            continue;
        };
        if raw.schema_version != EVENT_SCHEMA_VERSION
            || raw.sequence == 0
            || raw.session_time_ms > MAX_SESSION_MS
            || !safe_token(&raw.session_id, 64)
            || !safe_token(&raw.kind, 64)
            || !safe_optional_token(&raw.subject_ref, 64)
            || !safe_optional_token(&raw.target_ref, 64)
            || !safe_optional_token(&raw.state, 64)
            || raw.numeric_value.is_some_and(|value| !value.is_finite())
        {
            counts.malformed_events += 1;
            continue;
        }
        if raw.session_id != context.session_id {
            counts.wrong_session_events += 1;
            continue;
        }
        if raw.sequence <= last_sequence {
            counts.duplicate_or_reordered_events += 1;
            continue;
        }
        sequence_gaps = sequence_gaps
            .saturating_add(raw.sequence.saturating_sub(last_sequence.saturating_add(1)));
        if raw.session_time_ms < last_time {
            time_regressions += 1;
        }
        last_sequence = raw.sequence;
        last_time = raw.session_time_ms;

        if raw.kind == "session_start" {
            if observed_session_start || !events.is_empty() || observed_session_end {
                boundary_errors += 1;
            }
            observed_session_start = true;
        } else if raw.kind == "session_end" {
            if !observed_session_start || observed_session_end {
                boundary_errors += 1;
            }
            observed_session_end = true;
        } else if observed_session_end {
            boundary_errors += 1;
        }

        events.push(NormalizedEvent {
            session_id: raw.session_id,
            sequence: raw.sequence,
            session_time_ms: raw.session_time_ms,
            kind: raw.kind,
            subject_ref: raw.subject_ref,
            target_ref: raw.target_ref,
            state: raw.state,
            numeric_value: raw.numeric_value,
        });
        counts.accepted_events += 1;
    }

    let complete = observed_session_start
        && observed_session_end
        && boundary_errors == 0
        && sequence_gaps == 0
        && time_regressions == 0
        && counts.malformed_events == 0
        && counts.wrong_session_events == 0
        && counts.duplicate_or_reordered_events == 0
        && counts.oversized_lines == 0
        && counts.omitted_events == 0
        && !input_truncated;

    Ok(CaptureReport {
        schema_version: EVENT_SCHEMA_VERSION,
        context: context.clone(),
        counts,
        quality: CaptureQuality {
            complete,
            observed_session_start,
            observed_session_end,
            boundary_errors,
            sequence_gaps,
            time_regressions,
            input_truncated,
            source_verified: false,
        },
        events,
    })
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssertionSpec {
    pub id: String,
    pub event_kind: String,
    #[serde(default)]
    pub subject_ref: Option<String>,
    #[serde(default)]
    pub target_ref: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    pub min_count: u32,
    #[serde(default)]
    pub max_count: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssertionStatus {
    Passed,
    Failed,
    Inconclusive,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveUefnStatus {
    /// A trusted UEFN adapter must establish live provenance separately.
    Untested,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssertionResult {
    pub id: String,
    pub observed_count: u32,
    pub status: AssertionStatus,
    pub reason_code: &'static str,
    pub live_uefn_status: LiveUefnStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssertionEvaluation {
    pub results: Vec<AssertionResult>,
    pub omitted_specs: usize,
    pub source_kind: CaptureSourceKind,
    pub capture_complete: bool,
}

/// Evaluate project-defined event-count assertions against normalized local evidence.
/// A missing event fails only for a complete capture. Live UEFN status remains untested.
pub fn evaluate_assertions(report: &CaptureReport, specs: &[AssertionSpec]) -> AssertionEvaluation {
    let results = specs
        .iter()
        .take(MAX_ASSERTIONS)
        .map(|spec| {
            let valid = safe_token(&spec.id, 64)
                && safe_token(&spec.event_kind, 64)
                && safe_optional_token(&spec.subject_ref, 64)
                && safe_optional_token(&spec.target_ref, 64)
                && safe_optional_token(&spec.state, 64)
                && spec.max_count.is_none_or(|max| max >= spec.min_count);
            let count = if valid {
                report
                    .events
                    .iter()
                    .filter(|event| {
                        event.kind == spec.event_kind
                            && spec
                                .subject_ref
                                .as_ref()
                                .is_none_or(|value| event.subject_ref.as_ref() == Some(value))
                            && spec
                                .target_ref
                                .as_ref()
                                .is_none_or(|value| event.target_ref.as_ref() == Some(value))
                            && spec
                                .state
                                .as_ref()
                                .is_none_or(|value| event.state.as_ref() == Some(value))
                    })
                    .count() as u32
            } else {
                0
            };
            let (status, reason_code) = if !valid {
                (AssertionStatus::Invalid, "INVALID_ASSERTION_SPEC")
            } else if spec.max_count.is_some_and(|max| count > max) {
                (AssertionStatus::Failed, "COUNT_EXCEEDS_MAX")
            } else if report.quality.complete {
                if count >= spec.min_count {
                    (AssertionStatus::Passed, "COUNT_WITHIN_BOUNDS")
                } else {
                    (AssertionStatus::Failed, "COUNT_BELOW_MIN")
                }
            } else if spec.min_count > 0 && count >= spec.min_count && spec.max_count.is_none() {
                (
                    AssertionStatus::Passed,
                    "POSITIVE_WITNESS_IN_PARTIAL_CAPTURE",
                )
            } else {
                (AssertionStatus::Inconclusive, "CAPTURE_INCOMPLETE")
            };
            AssertionResult {
                id: if safe_token(&spec.id, 64) {
                    spec.id.clone()
                } else {
                    "invalid_spec".to_string()
                },
                observed_count: count,
                status,
                reason_code,
                live_uefn_status: LiveUefnStatus::Untested,
            }
        })
        .collect();
    AssertionEvaluation {
        results,
        omitted_specs: specs.len().saturating_sub(MAX_ASSERTIONS),
        source_kind: report.context.source_kind,
        capture_complete: report.quality.complete,
    }
}

fn safe_optional_token(value: &Option<String>, max_len: usize) -> bool {
    value
        .as_ref()
        .is_none_or(|value| safe_token(value, max_len))
}

fn safe_token(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}
