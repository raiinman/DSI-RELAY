use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq,
    PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    Public,
    Project,
    Sensitive,
    Credential,
}

impl DataClass {
    pub fn strongest(
        values: impl IntoIterator<Item = DataClass>,
    ) -> DataClass {
        values.into_iter().max().unwrap_or(DataClass::Public)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CredentialHandle {
    pub id: String,
    pub integration: String,
    pub scopes: BTreeSet<String>,
    pub status: String,
}

impl CredentialHandle {
    pub fn active(
        id: impl Into<String>,
        integration: impl Into<String>,
        scopes: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            id: id.into(),
            integration: integration.into(),
            scopes: scopes.into_iter().collect(),
            status: "active".to_string(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == "active"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DestinationPolicy {
    pub remote: bool,
    pub allowed_classes: BTreeSet<DataClass>,
    pub allowed_modalities: BTreeSet<String>,
    pub allowed_projects: Option<BTreeSet<String>>,
    pub max_bytes: Option<u64>,
}

impl DestinationPolicy {
    pub fn local() -> Self {
        Self {
            remote: false,
            allowed_classes: [
                DataClass::Public,
                DataClass::Project,
                DataClass::Sensitive,
            ]
            .into_iter()
            .collect(),
            allowed_modalities: BTreeSet::new(),
            allowed_projects: None,
            max_bytes: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EgressPolicy {
    pub local_only: bool,
    pub destinations: BTreeMap<String, DestinationPolicy>,
}

impl Default for EgressPolicy {
    fn default() -> Self {
        let mut destinations = BTreeMap::new();
        destinations.insert("local".to_string(), DestinationPolicy::local());
        Self {
            local_only: true,
            destinations,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EgressRequest {
    pub destination: String,
    pub project_id: Option<String>,
    pub data_classes: Vec<DataClass>,
    pub modalities: Vec<String>,
    pub approx_bytes: u64,
    pub source_refs: Vec<String>,
    pub purpose: String,
    pub credential_handle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EgressDecision {
    pub allowed: bool,
    pub reason: String,
    pub strongest_class: DataClass,
    pub destination: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityError {
    pub code: &'static str,
    pub message: String,
}

impl AuthorityError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for AuthorityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AuthorityError {}

#[derive(Debug, Clone)]
pub struct ExecutionAuthority {
    pub actor_id: String,
    pub client_id: String,
    pub delegator_id: Option<String>,
    pub permissions: BTreeSet<String>,
    pub effect_classes: BTreeSet<String>,
    pub project_ids: Option<BTreeSet<String>>,
    pub credential_handles: BTreeMap<String, CredentialHandle>,
    pub egress: EgressPolicy,
}

impl ExecutionAuthority {
    pub fn local_user(client_id: impl Into<String>) -> Self {
        Self {
            actor_id: "local-user".to_string(),
            client_id: client_id.into(),
            delegator_id: None,
            permissions: ["read", "state_write", "host_control"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            effect_classes: [
                "observe",
                "analyze",
                "relay_self_repair",
                "relay_state_write",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            project_ids: None,
            credential_handles: BTreeMap::new(),
            egress: EgressPolicy::default(),
        }
    }

    pub fn require_permission(&self, permission: &str) -> Result<(), AuthorityError> {
        if self.permissions.contains(permission) {
            Ok(())
        } else {
            Err(AuthorityError::new(
                "PERMISSION_DENIED",
                format!("permission {permission} is not granted"),
            ))
        }
    }

    pub fn require_effect(&self, effect: &str) -> Result<(), AuthorityError> {
        if self.effect_classes.contains(effect) {
            Ok(())
        } else {
            Err(AuthorityError::new(
                "EFFECT_NOT_ALLOWED",
                format!("effect class {effect} is not granted"),
            ))
        }
    }

    pub fn require_project(
        &self,
        project_id: Option<&str>,
    ) -> Result<(), AuthorityError> {
        let Some(allowed) = &self.project_ids else {
            return Ok(());
        };
        let Some(project_id) = project_id else {
            return Err(AuthorityError::new(
                "PROJECT_SCOPE_REQUIRED",
                "project-scoped authority requires an explicit project ID",
            ));
        };
        if allowed.contains(project_id) {
            Ok(())
        } else {
            Err(AuthorityError::new(
                "PROJECT_SCOPE_DENIED",
                format!("project {project_id} is outside the granted scope"),
            ))
        }
    }

    pub fn require_credential_handle(
        &self,
        handle_id: &str,
        scope: Option<&str>,
    ) -> Result<&CredentialHandle, AuthorityError> {
        let handle = self.credential_handles.get(handle_id).ok_or_else(|| {
            AuthorityError::new(
                "CREDENTIAL_HANDLE_DENIED",
                "credential handle is not granted to this execution",
            )
        })?;
        if !handle.is_active() {
            return Err(AuthorityError::new(
                "CREDENTIAL_HANDLE_REVOKED",
                "credential handle is not active",
            ));
        }
        if let Some(scope) = scope {
            if !handle.scopes.contains(scope) {
                return Err(AuthorityError::new(
                    "CREDENTIAL_SCOPE_DENIED",
                    format!("credential handle does not grant scope {scope}"),
                ));
            }
        }
        Ok(handle)
    }

    pub fn evaluate_egress(
        &self,
        request: &EgressRequest,
    ) -> EgressDecision {
        let strongest = DataClass::strongest(
            request.data_classes.iter().copied(),
        );

        if strongest == DataClass::Credential {
            return EgressDecision {
                allowed: false,
                reason: "credential data cannot be egress payload".to_string(),
                strongest_class: strongest,
                destination: request.destination.clone(),
            };
        }

        let Some(policy) = self.egress.destinations.get(&request.destination)
        else {
            return EgressDecision {
                allowed: false,
                reason: "destination is not configured".to_string(),
                strongest_class: strongest,
                destination: request.destination.clone(),
            };
        };

        if self.egress.local_only && policy.remote {
            return EgressDecision {
                allowed: false,
                reason: "local-only policy blocks remote project-data egress".to_string(),
                strongest_class: strongest,
                destination: request.destination.clone(),
            };
        }

        if !policy.allowed_classes.contains(&strongest) {
            return EgressDecision {
                allowed: false,
                reason: format!(
                    "data class {:?} is not allowed for destination",
                    strongest
                ),
                strongest_class: strongest,
                destination: request.destination.clone(),
            };
        }

        if let Some(projects) = &policy.allowed_projects {
            match request.project_id.as_deref() {
                Some(project) if projects.contains(project) => {}
                _ => {
                    return EgressDecision {
                        allowed: false,
                        reason: "project is outside destination scope".to_string(),
                        strongest_class: strongest,
                        destination: request.destination.clone(),
                    };
                }
            }
        }

        if !policy.allowed_modalities.is_empty()
            && request
                .modalities
                .iter()
                .any(|value| !policy.allowed_modalities.contains(value))
        {
            return EgressDecision {
                allowed: false,
                reason: "one or more modalities are not allowed".to_string(),
                strongest_class: strongest,
                destination: request.destination.clone(),
            };
        }

        if let Some(max_bytes) = policy.max_bytes {
            if request.approx_bytes > max_bytes {
                return EgressDecision {
                    allowed: false,
                    reason: format!(
                        "payload exceeds destination byte budget {max_bytes}"
                    ),
                    strongest_class: strongest,
                    destination: request.destination.clone(),
                };
            }
        }

        if let Some(handle_id) = request.credential_handle.as_deref() {
            if let Err(error) =
                self.require_credential_handle(handle_id, Some("egress"))
            {
                return EgressDecision {
                    allowed: false,
                    reason: error.message,
                    strongest_class: strongest,
                    destination: request.destination.clone(),
                };
            }
        }

        EgressDecision {
            allowed: true,
            reason: "egress request is within configured policy".to_string(),
            strongest_class: strongest,
            destination: request.destination.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_class_propagation_uses_strongest_source() {
        assert_eq!(
            DataClass::strongest([
                DataClass::Public,
                DataClass::Sensitive,
                DataClass::Project,
            ]),
            DataClass::Sensitive
        );
    }

    #[test]
    fn local_only_blocks_remote_egress() {
        let mut authority = ExecutionAuthority::local_user("test");
        authority.egress.destinations.insert(
            "remote-ai".to_string(),
            DestinationPolicy {
                remote: true,
                allowed_classes: [DataClass::Project].into_iter().collect(),
                allowed_modalities: ["text".to_string()].into_iter().collect(),
                allowed_projects: None,
                max_bytes: Some(1024),
            },
        );

        let decision = authority.evaluate_egress(&EgressRequest {
            destination: "remote-ai".to_string(),
            project_id: Some("PRJ-fixture".to_string()),
            data_classes: vec![DataClass::Project],
            modalities: vec!["text".to_string()],
            approx_bytes: 100,
            source_refs: vec!["RES-fixture".to_string()],
            purpose: "diagnose".to_string(),
            credential_handle: None,
        });
        assert!(!decision.allowed);
        assert!(decision.reason.contains("local-only"));
    }

    #[test]
    fn credentials_are_handles_not_egress_payload() {
        let authority = ExecutionAuthority::local_user("test");
        let decision = authority.evaluate_egress(&EgressRequest {
            destination: "local".to_string(),
            project_id: None,
            data_classes: vec![DataClass::Credential],
            modalities: vec!["text".to_string()],
            approx_bytes: 1,
            source_refs: vec![],
            purpose: "fixture".to_string(),
            credential_handle: None,
        });
        assert!(!decision.allowed);
        assert!(decision.reason.contains("credential"));
    }

    #[test]
    fn project_scope_fails_closed() {
        let mut authority = ExecutionAuthority::local_user("test");
        authority.project_ids =
            Some(["PRJ-allowed".to_string()].into_iter().collect());
        assert!(authority
            .require_project(Some("PRJ-allowed"))
            .is_ok());
        let error = authority
            .require_project(Some("PRJ-other"))
            .unwrap_err();
        assert_eq!(error.code, "PROJECT_SCOPE_DENIED");
    }
}
