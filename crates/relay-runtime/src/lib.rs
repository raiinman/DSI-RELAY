//! Runtime state and evidence eligibility without UEFN side effects.

use relay_verse::{CaptureReport, CaptureSourceKind, MAX_EVENTS};
use serde::Serialize;

pub const MAX_PROBES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionPhase {
    Disconnected,
    Launching,
    Connected,
    Running,
    Stopping,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    LaunchRequested,
    EditorConnected,
    PlayStarted,
    StopRequested,
    PlayEnded,
    LaunchFailed,
    ConnectionLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeState {
    AwaitingCapture,
    Witnessed,
    Absent,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveUefnStatus {
    Untested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeError {
    InvalidIdentity,
    InvalidTransition,
    ProbeLimit,
    DuplicateProbe,
    CaptureMismatch,
    CaptureInvalid,
    CaptureAlreadyAttached,
}

impl RuntimeError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidIdentity => "INVALID_RUNTIME_IDENTITY",
            Self::InvalidTransition => "INVALID_SESSION_TRANSITION",
            Self::ProbeLimit => "PROBE_LIMIT",
            Self::DuplicateProbe => "DUPLICATE_PROBE",
            Self::CaptureMismatch => "CAPTURE_CONTEXT_MISMATCH",
            Self::CaptureInvalid => "CAPTURE_INVALID",
            Self::CaptureAlreadyAttached => "CAPTURE_ALREADY_ATTACHED",
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeSnapshot {
    pub id: String,
    pub event_kind: String,
    pub state: ProbeState,
    pub observed_count: u32,
    pub positive_assertion_eligible: bool,
    pub negative_assertion_eligible: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureSnapshot {
    pub source_kind: CaptureSourceKind,
    pub source_version: String,
    pub capture_ref: String,
    pub complete: bool,
    pub accepted_events: usize,
    pub rejected_events: usize,
    pub source_identity_verified: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSnapshot {
    pub format_version: u32,
    pub project_id: String,
    pub session_id: String,
    pub expected_source_version: String,
    pub phase: SessionPhase,
    pub transition_count: u32,
    pub interrupted: bool,
    pub capture: Option<CaptureSnapshot>,
    pub probes: Vec<ProbeSnapshot>,
    pub live_uefn_status: LiveUefnStatus,
}

#[derive(Debug, Clone)]
struct Probe {
    id: String,
    event_kind: String,
    state: ProbeState,
    observed_count: u32,
    positive_assertion_eligible: bool,
    negative_assertion_eligible: bool,
}

/// One project-scoped play-session attempt. A finished or interrupted tracker
/// is never reset; start a new tracker with a new session ID instead.
pub struct RuntimeSession {
    project_id: String,
    session_id: String,
    expected_source_version: String,
    phase: SessionPhase,
    transition_count: u32,
    ever_launched: bool,
    interrupted: bool,
    clean_end: bool,
    capture: Option<CaptureSnapshot>,
    probes: Vec<Probe>,
}

impl RuntimeSession {
    pub fn new(
        project_id: &str,
        session_id: &str,
        expected_source_version: &str,
    ) -> Result<Self, RuntimeError> {
        if !safe_token(project_id, 64)
            || !safe_token(session_id, 64)
            || !safe_token(expected_source_version, 64)
        {
            return Err(RuntimeError::InvalidIdentity);
        }
        Ok(Self {
            project_id: project_id.to_owned(),
            session_id: session_id.to_owned(),
            expected_source_version: expected_source_version.to_owned(),
            phase: SessionPhase::Disconnected,
            transition_count: 0,
            ever_launched: false,
            interrupted: false,
            clean_end: false,
            capture: None,
            probes: Vec::new(),
        })
    }

    pub fn phase(&self) -> SessionPhase {
        self.phase
    }

    pub fn transition(&mut self, event: SessionEvent) -> Result<SessionPhase, RuntimeError> {
        use SessionEvent as Event;
        use SessionPhase as Phase;
        let next = match (self.phase, event) {
            (Phase::Disconnected, Event::LaunchRequested) if !self.ever_launched => {
                self.ever_launched = true;
                Phase::Launching
            }
            (Phase::Launching, Event::EditorConnected) => Phase::Connected,
            (Phase::Connected, Event::PlayStarted) => Phase::Running,
            (Phase::Running, Event::StopRequested) => Phase::Stopping,
            (Phase::Running | Phase::Stopping, Event::PlayEnded) => {
                self.clean_end = true;
                Phase::Ended
            }
            (Phase::Launching, Event::LaunchFailed) => Phase::Ended,
            (
                Phase::Launching | Phase::Connected | Phase::Running | Phase::Stopping,
                Event::ConnectionLost,
            ) => {
                self.interrupted = true;
                Phase::Disconnected
            }
            _ => return Err(RuntimeError::InvalidTransition),
        };
        self.phase = next;
        self.transition_count += 1;
        Ok(next)
    }

    /// Register before play begins so absence can be interpreted only against
    /// a complete capture of the whole session.
    pub fn register_probe(&mut self, id: &str, event_kind: &str) -> Result<(), RuntimeError> {
        if !safe_token(id, 64) || !safe_token(event_kind, 64) {
            return Err(RuntimeError::InvalidIdentity);
        }
        if !matches!(
            self.phase,
            SessionPhase::Disconnected | SessionPhase::Launching | SessionPhase::Connected
        ) || self.interrupted
        {
            return Err(RuntimeError::InvalidTransition);
        }
        if self.probes.len() >= MAX_PROBES {
            return Err(RuntimeError::ProbeLimit);
        }
        if self.probes.iter().any(|probe| probe.id == id) {
            return Err(RuntimeError::DuplicateProbe);
        }
        self.probes.push(Probe {
            id: id.to_owned(),
            event_kind: event_kind.to_owned(),
            state: ProbeState::AwaitingCapture,
            observed_count: 0,
            positive_assertion_eligible: false,
            negative_assertion_eligible: false,
        });
        Ok(())
    }

    /// Attach a normalized capture after a clean play end. Its source kind is
    /// caller-declared and never upgrades the live UEFN status.
    pub fn attach_capture(&mut self, report: &CaptureReport) -> Result<(), RuntimeError> {
        if self.phase != SessionPhase::Ended || !self.clean_end || self.interrupted {
            return Err(RuntimeError::InvalidTransition);
        }
        if self.capture.is_some() {
            return Err(RuntimeError::CaptureAlreadyAttached);
        }
        if report.context.session_id != self.session_id
            || report.context.source_version != self.expected_source_version
            || report
                .events
                .iter()
                .any(|event| event.session_id != self.session_id)
        {
            return Err(RuntimeError::CaptureMismatch);
        }
        if !safe_token(&report.context.capture_ref, 96)
            || report.events.len() > MAX_EVENTS
            || report.counts.accepted_events != report.events.len()
            || (report.quality.complete && !capture_complete_invariants(report))
        {
            return Err(RuntimeError::CaptureInvalid);
        }
        let complete = report.quality.complete;
        let rejected_events = report.counts.malformed_events
            + report.counts.wrong_session_events
            + report.counts.duplicate_or_reordered_events
            + report.counts.oversized_lines
            + report.counts.omitted_events;
        for probe in &mut self.probes {
            let count = report
                .events
                .iter()
                .filter(|event| event.kind == probe.event_kind)
                .count();
            probe.observed_count = count.min(u32::MAX as usize) as u32;
            probe.positive_assertion_eligible = count > 0;
            probe.negative_assertion_eligible = complete;
            probe.state = if count > 0 {
                ProbeState::Witnessed
            } else if complete {
                ProbeState::Absent
            } else {
                ProbeState::Inconclusive
            };
        }
        self.capture = Some(CaptureSnapshot {
            source_kind: report.context.source_kind,
            source_version: report.context.source_version.clone(),
            capture_ref: report.context.capture_ref.clone(),
            complete,
            accepted_events: report.counts.accepted_events,
            rejected_events,
            // The capture parser cannot verify a creator-app session. Even a
            // caller-modified quality flag is not an adapter attestation.
            source_identity_verified: false,
        });
        Ok(())
    }

    pub fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            format_version: 1,
            project_id: self.project_id.clone(),
            session_id: self.session_id.clone(),
            expected_source_version: self.expected_source_version.clone(),
            phase: self.phase,
            transition_count: self.transition_count,
            interrupted: self.interrupted,
            capture: self.capture.clone(),
            probes: self
                .probes
                .iter()
                .map(|probe| ProbeSnapshot {
                    id: probe.id.clone(),
                    event_kind: probe.event_kind.clone(),
                    state: probe.state,
                    observed_count: probe.observed_count,
                    positive_assertion_eligible: probe.positive_assertion_eligible,
                    negative_assertion_eligible: probe.negative_assertion_eligible,
                })
                .collect(),
            live_uefn_status: LiveUefnStatus::Untested,
        }
    }
}

fn capture_complete_invariants(report: &CaptureReport) -> bool {
    let quality = &report.quality;
    let counts = &report.counts;
    quality.observed_session_start
        && quality.observed_session_end
        && quality.boundary_errors == 0
        && quality.sequence_gaps == 0
        && quality.time_regressions == 0
        && !quality.input_truncated
        && counts.malformed_events == 0
        && counts.wrong_session_events == 0
        && counts.duplicate_or_reordered_events == 0
        && counts.oversized_lines == 0
        && counts.omitted_events == 0
        && report
            .events
            .first()
            .is_some_and(|event| event.kind == "session_start")
        && report
            .events
            .last()
            .is_some_and(|event| event.kind == "session_end")
        && report
            .events
            .iter()
            .filter(|event| event.kind == "session_start")
            .count()
            == 1
        && report
            .events
            .iter()
            .filter(|event| event.kind == "session_end")
            .count()
            == 1
        && report
            .events
            .iter()
            .enumerate()
            .all(|(index, event)| event.sequence == index as u64 + 1)
        && report
            .events
            .windows(2)
            .all(|pair| pair[0].session_time_ms <= pair[1].session_time_ms)
}

fn safe_token(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_verse::{CaptureContext, normalize_capture};

    fn capture(version: &str, complete: bool) -> CaptureReport {
        let context = CaptureContext {
            session_id: "session-1".to_owned(),
            source_kind: CaptureSourceKind::SyntheticFixture,
            source_version: version.to_owned(),
            capture_ref: "capture-1".to_owned(),
        };
        let mut text = String::new();
        for (sequence, kind) in [(1, "session_start"), (2, "hit"), (3, "session_end")] {
            if !complete && kind == "session_end" {
                break;
            }
            text.push_str(&format!("RELAY_EVENT_V1 {{\"schema_version\":1,\"session_id\":\"session-1\",\"sequence\":{sequence},\"session_time_ms\":{sequence},\"kind\":\"{kind}\"}}\n"));
        }
        normalize_capture(&context, &text).unwrap()
    }

    fn ended_session() -> RuntimeSession {
        let mut session = RuntimeSession::new("project-1", "session-1", "adapter-v1").unwrap();
        session.register_probe("hit-probe", "hit").unwrap();
        session.register_probe("miss-probe", "miss").unwrap();
        for event in [
            SessionEvent::LaunchRequested,
            SessionEvent::EditorConnected,
            SessionEvent::PlayStarted,
            SessionEvent::StopRequested,
            SessionEvent::PlayEnded,
        ] {
            session.transition(event).unwrap();
        }
        session
    }

    #[test]
    fn complete_synthetic_capture_supports_local_evidence_but_never_live_pass() {
        let mut session = ended_session();
        let mut report = capture("adapter-v1", true);
        report.quality.source_verified = true; // caller-controlled flag is not trusted attestation
        session.attach_capture(&report).unwrap();
        let snapshot = session.snapshot();
        assert_eq!(snapshot.phase, SessionPhase::Ended);
        assert_eq!(snapshot.live_uefn_status, LiveUefnStatus::Untested);
        assert!(!snapshot.capture.unwrap().source_identity_verified);
        assert_eq!(snapshot.probes[0].state, ProbeState::Witnessed);
        assert_eq!(snapshot.probes[1].state, ProbeState::Absent);
        assert!(snapshot.probes[1].negative_assertion_eligible);
    }

    #[test]
    fn incomplete_capture_cannot_prove_absence_and_version_mismatch_is_rejected() {
        let mut session = ended_session();
        assert_eq!(
            session.attach_capture(&capture("wrong-v2", true)),
            Err(RuntimeError::CaptureMismatch)
        );
        assert!(session.snapshot().capture.is_none());
        session
            .attach_capture(&capture("adapter-v1", false))
            .unwrap();
        let snapshot = session.snapshot();
        assert_eq!(snapshot.probes[1].state, ProbeState::Inconclusive);
        assert!(!snapshot.probes[1].negative_assertion_eligible);
        assert!(snapshot.probes[0].positive_assertion_eligible);
    }

    #[test]
    fn lost_connection_cannot_be_reused_or_accept_capture() {
        let mut session = RuntimeSession::new("project-1", "session-1", "adapter-v1").unwrap();
        session.transition(SessionEvent::LaunchRequested).unwrap();
        session.transition(SessionEvent::EditorConnected).unwrap();
        session.transition(SessionEvent::ConnectionLost).unwrap();
        assert_eq!(
            session.transition(SessionEvent::LaunchRequested),
            Err(RuntimeError::InvalidTransition)
        );
        assert_eq!(
            session.attach_capture(&capture("adapter-v1", true)),
            Err(RuntimeError::InvalidTransition)
        );
        assert!(session.snapshot().interrupted);
    }

    #[test]
    fn forged_completeness_is_rejected_without_changing_session() {
        let mut session = ended_session();
        let mut report = capture("adapter-v1", false);
        report.quality.complete = true;
        assert_eq!(session.attach_capture(&report), Err(RuntimeError::CaptureInvalid));
        let snapshot = session.snapshot();
        assert!(snapshot.capture.is_none());
        assert_eq!(snapshot.probes[0].state, ProbeState::AwaitingCapture);
    }
}
