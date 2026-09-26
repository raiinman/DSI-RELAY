use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::OnceLock;

pub const REGISTRY_SOURCE: &str =
    include_str!("../commands.registry.json");
pub const REGISTRY_FORMAT: u32 = 1;
pub const JSON_SCHEMA_DIALECT: &str =
    "https://json-schema.org/draft/2020-12/schema";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRegistry {
    pub registry_format: u32,
    pub registry_id: String,
    pub schema_dialect: String,
    pub compatibility: Value,
    pub reserved_command_ids: Vec<String>,
    pub deprecated_command_ids: Vec<String>,
    #[serde(default)]
    pub common_errors: Vec<String>,
    pub error_schema: Value,
    pub commands: Vec<CommandSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub id: String,
    pub version: u32,
    pub stability: String,
    pub summary: String,
    pub effect_class: String,
    pub permission: String,
    pub idempotency: String,
    pub surfaces: Vec<String>,
    pub arguments_schema: Value,
    pub result_schema: Value,
    pub errors: Vec<String>,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryError {
    pub code: &'static str,
    pub message: String,
}

impl RegistryError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RegistryError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

static REGISTRY: OnceLock<CommandRegistry> = OnceLock::new();

fn registry_source() -> &'static str {
    REGISTRY_SOURCE
        .strip_prefix('\u{feff}')
        .unwrap_or(REGISTRY_SOURCE)
}

pub fn registry() -> &'static CommandRegistry {
    REGISTRY.get_or_init(|| {
        let parsed: CommandRegistry =
            serde_json::from_str(registry_source()).expect("embedded command registry must parse");
        validate_registry(&parsed).expect("embedded command registry must validate");
        parsed
    })
}

pub fn validate_embedded_registry() -> Result<(), RegistryError> {
    let parsed: CommandRegistry = serde_json::from_str(registry_source())
        .map_err(|error| RegistryError::new(
            "REGISTRY_INVALID",
            format!("parse command registry: {error}"),
        ))?;
    validate_registry(&parsed)
}

pub fn validate_registry(registry: &CommandRegistry) -> Result<(), RegistryError> {
    if registry.registry_format != REGISTRY_FORMAT {
        return Err(RegistryError::new(
            "REGISTRY_FORMAT_INCOMPATIBLE",
            format!(
                "registry format {} is unsupported; expected {}",
                registry.registry_format, REGISTRY_FORMAT
            ),
        ));
    }
    if registry.schema_dialect != JSON_SCHEMA_DIALECT {
        return Err(RegistryError::new(
            "REGISTRY_SCHEMA_DIALECT_UNSUPPORTED",
            format!("unsupported schema dialect: {}", registry.schema_dialect),
        ));
    }
    validate_schema_definition(&registry.error_schema, "$error")?;

    let reserved: BTreeSet<_> = registry.reserved_command_ids.iter().collect();
    let deprecated: BTreeSet<_> = registry.deprecated_command_ids.iter().collect();

    if reserved.len() != registry.reserved_command_ids.len() {
        return Err(RegistryError::new(
            "REGISTRY_INVALID",
            "reserved command IDs must be unique",
        ));
    }
    if deprecated.len() != registry.deprecated_command_ids.len() {
        return Err(RegistryError::new(
            "REGISTRY_INVALID",
            "deprecated command IDs must be unique",
        ));
    }
    if reserved.iter().any(|id| deprecated.contains(id)) {
        return Err(RegistryError::new(
            "REGISTRY_INVALID",
            "an ID cannot be both reserved and deprecated",
        ));
    }

    let mut pairs = BTreeSet::new();
    for command in &registry.commands {
        if command.id.trim().is_empty() || command.version == 0 {
            return Err(RegistryError::new(
                "REGISTRY_INVALID",
                "command IDs must be non-empty and versions must be >= 1",
            ));
        }
        if reserved.contains(&command.id) || deprecated.contains(&command.id) {
            return Err(RegistryError::new(
                "REGISTRY_ID_REUSE",
                format!("active command reuses reserved/deprecated ID {}", command.id),
            ));
        }
        if !pairs.insert((command.id.clone(), command.version)) {
            return Err(RegistryError::new(
                "REGISTRY_INVALID",
                format!("duplicate command contract {}@{}", command.id, command.version),
            ));
        }
        validate_schema_definition(&command.arguments_schema, "$arguments")?;
        validate_schema_definition(&command.result_schema, "$result")?;
    }
    Ok(())
}


fn validate_schema_definition(schema: &Value, path: &str) -> Result<(), RegistryError> {
    if schema.is_boolean() {
        return Ok(());
    }
    let Some(object) = schema.as_object() else {
        return Err(RegistryError::new(
            "REGISTRY_SCHEMA_UNSUPPORTED",
            format!("{path} schema must be an object or boolean"),
        ));
    };

    const ALLOWED: &[&str] = &[
        "$schema",
        "title",
        "description",
        "type",
        "properties",
        "required",
        "additionalProperties",
        "items",
        "enum",
        "const",
        "minLength",
        "minimum",
        "maximum",
    ];
    for key in object.keys() {
        if !ALLOWED.contains(&key.as_str()) {
            return Err(RegistryError::new(
                "REGISTRY_SCHEMA_UNSUPPORTED",
                format!("{path} uses unsupported JSON Schema keyword {key}"),
            ));
        }
    }

    if let Some(types) = object.get("type") {
        validate_type_declaration(types, path)?;
    }

    if let Some(properties) = object.get("properties") {
        let Some(properties) = properties.as_object() else {
            return Err(RegistryError::new(
                "REGISTRY_SCHEMA_UNSUPPORTED",
                format!("{path}.properties must be an object"),
            ));
        };
        for (name, child) in properties {
            validate_schema_definition(child, &format!("{path}.properties.{name}"))?;
        }
    }

    if let Some(required) = object.get("required") {
        let Some(required) = required.as_array() else {
            return Err(RegistryError::new(
                "REGISTRY_SCHEMA_UNSUPPORTED",
                format!("{path}.required must be an array"),
            ));
        };
        let mut names = BTreeSet::new();
        for value in required {
            let Some(name) = value.as_str() else {
                return Err(RegistryError::new(
                    "REGISTRY_SCHEMA_UNSUPPORTED",
                    format!("{path}.required entries must be strings"),
                ));
            };
            if !names.insert(name) {
                return Err(RegistryError::new(
                    "REGISTRY_SCHEMA_UNSUPPORTED",
                    format!("{path}.required contains duplicate {name}"),
                ));
            }
        }
    }

    if let Some(additional) = object.get("additionalProperties") {
        if !additional.is_boolean() {
            return Err(RegistryError::new(
                "REGISTRY_SCHEMA_UNSUPPORTED",
                format!("{path}.additionalProperties must be boolean in RELAY registry v1"),
            ));
        }
    }

    if let Some(items) = object.get("items") {
        validate_schema_definition(items, &format!("{path}.items"))?;
    }

    if let Some(values) = object.get("enum") {
        if !values.is_array() {
            return Err(RegistryError::new(
                "REGISTRY_SCHEMA_UNSUPPORTED",
                format!("{path}.enum must be an array"),
            ));
        }
    }

    if let Some(min_length) = object.get("minLength") {
        if min_length.as_u64().is_none() {
            return Err(RegistryError::new(
                "REGISTRY_SCHEMA_UNSUPPORTED",
                format!("{path}.minLength must be a non-negative integer"),
            ));
        }
    }

    for keyword in ["minimum", "maximum"] {
        if let Some(value) = object.get(keyword) {
            if !value.is_number() {
                return Err(RegistryError::new(
                    "REGISTRY_SCHEMA_UNSUPPORTED",
                    format!("{path}.{keyword} must be numeric"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_type_declaration(types: &Value, path: &str) -> Result<(), RegistryError> {
    let allowed = ["object", "array", "string", "integer", "number", "boolean", "null"];
    let values: Vec<&str> = if let Some(value) = types.as_str() {
        vec![value]
    } else if let Some(values) = types.as_array() {
        values
            .iter()
            .map(|value| value.as_str().unwrap_or(""))
            .collect()
    } else {
        return Err(RegistryError::new(
            "REGISTRY_SCHEMA_UNSUPPORTED",
            format!("{path}.type must be a string or array of strings"),
        ));
    };
    if values.is_empty() || values.iter().any(|value| !allowed.contains(value)) {
        return Err(RegistryError::new(
            "REGISTRY_SCHEMA_UNSUPPORTED",
            format!("{path}.type contains an unsupported JSON type"),
        ));
    }
    Ok(())
}


pub fn validate_value(schema: &Value, value: &Value) -> Result<(), ValidationError> {
    validate_value_at(schema, value, "$")
}

fn validate_value_at(
    schema: &Value,
    value: &Value,
    path: &str,
) -> Result<(), ValidationError> {
    if let Some(allow) = schema.as_bool() {
        return if allow {
            Ok(())
        } else {
            Err(ValidationError {
                path: path.to_string(),
                message: "value rejected by false schema".to_string(),
            })
        };
    }

    let object = schema.as_object().ok_or_else(|| ValidationError {
        path: path.to_string(),
        message: "invalid embedded schema".to_string(),
    })?;
    if object.is_empty() {
        return Ok(());
    }

    if let Some(expected) = object.get("const") {
        if value != expected {
            return Err(ValidationError {
                path: path.to_string(),
                message: format!("expected constant {}", compact(expected)),
            });
        }
    }

    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        if !values.iter().any(|candidate| candidate == value) {
            return Err(ValidationError {
                path: path.to_string(),
                message: "value is not one of the allowed enum values".to_string(),
            });
        }
    }

    if let Some(types) = object.get("type") {
        if !matches_declared_type(types, value) {
            return Err(ValidationError {
                path: path.to_string(),
                message: format!(
                    "expected type {}, got {}",
                    compact(types),
                    json_type(value)
                ),
            });
        }
    }

    if let Some(min_length) = object.get("minLength").and_then(Value::as_u64) {
        if let Some(text) = value.as_str() {
            if text.chars().count() < min_length as usize {
                return Err(ValidationError {
                    path: path.to_string(),
                    message: format!("string must contain at least {min_length} characters"),
                });
            }
        }
    }

    if let Some(minimum) = object.get("minimum").and_then(Value::as_f64) {
        if let Some(number) = value.as_f64() {
            if number < minimum {
                return Err(ValidationError {
                    path: path.to_string(),
                    message: format!("number must be >= {minimum}"),
                });
            }
        }
    }

    if let Some(maximum) = object.get("maximum").and_then(Value::as_f64) {
        if let Some(number) = value.as_f64() {
            if number > maximum {
                return Err(ValidationError {
                    path: path.to_string(),
                    message: format!("number must be <= {maximum}"),
                });
            }
        }
    }

    if let Some(instance) = value.as_object() {
        if let Some(required) = object.get("required").and_then(Value::as_array) {
            for name in required.iter().filter_map(Value::as_str) {
                if !instance.contains_key(name) {
                    return Err(ValidationError {
                        path: path.to_string(),
                        message: format!("missing required property {name}"),
                    });
                }
            }
        }

        let properties = object
            .get("properties")
            .and_then(Value::as_object);
        if let Some(properties) = properties {
            for (name, child_schema) in properties {
                if let Some(child) = instance.get(name) {
                    validate_value_at(
                        child_schema,
                        child,
                        &format!("{path}.{}", escape_path(name)),
                    )?;
                }
            }
        }

        if object.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
            let empty = Map::new();
            let properties = properties.unwrap_or(&empty);
            for name in instance.keys() {
                if !properties.contains_key(name) {
                    return Err(ValidationError {
                        path: format!("{path}.{}", escape_path(name)),
                        message: "additional property is not allowed".to_string(),
                    });
                }
            }
        }
    }

    if let Some(items_schema) = object.get("items") {
        if let Some(items) = value.as_array() {
            for (index, item) in items.iter().enumerate() {
                validate_value_at(
                    items_schema,
                    item,
                    &format!("{path}[{index}]"),
                )?;
            }
        }
    }

    Ok(())
}

fn matches_declared_type(types: &Value, value: &Value) -> bool {
    if let Some(expected) = types.as_str() {
        return matches_json_type(expected, value);
    }
    types
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .any(|expected| matches_json_type(expected, value))
        })
        .unwrap_or(false)
}

fn matches_json_type(expected: &str, value: &Value) -> bool {
    match expected {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    }
}


fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn compact(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<json>".to_string())
}

fn escape_path(value: &str) -> String {
    if value.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        value.to_string()
    } else {
        format!("[{}]", compact(&Value::String(value.to_string())))
    }
}

#[derive(Debug, Clone)]
pub enum ResolveError {
    UnknownCommand,
    VersionIncompatible { supported: Vec<u32> },
}

pub fn resolve_command(
    id: &str,
    requested_version: Option<u32>,
) -> Result<&'static CommandSpec, ResolveError> {
    let registry = registry();
    let mut matches: Vec<&CommandSpec> = registry
        .commands
        .iter()
        .filter(|command| command.id == id)
        .collect();
    if matches.is_empty() {
        return Err(ResolveError::UnknownCommand);
    }
    matches.sort_by_key(|command| command.version);

    if let Some(version) = requested_version {
        return matches
            .into_iter()
            .find(|command| command.version == version)
            .ok_or_else(|| ResolveError::VersionIncompatible {
                supported: registry
                    .commands
                    .iter()
                    .filter(|command| command.id == id)
                    .map(|command| command.version)
                    .collect(),
            });
    }
    Ok(*matches.last().expect("matches is non-empty"))
}

pub fn is_error_allowed(command: &CommandSpec, code: &str) -> bool {
    matches!(code, "VALIDATION_FAILED" | "RESULT_SCHEMA_VIOLATION")
        || registry()
            .common_errors
            .iter()
            .any(|allowed| allowed == code)
        || command.errors.iter().any(|allowed| allowed == code)
}

pub fn capability_ids() -> Vec<String> {
    let mut capabilities: Vec<String> = registry()
        .commands
        .iter()
        .map(|command| format!("{}@{}", command.id, command.version))
        .collect();
    capabilities.sort();
    capabilities.dedup();
    capabilities
}

pub fn is_surface_exposed(id: &str, surface: &str) -> bool {
    resolve_command(id, None)
        .map(|command| command.surfaces.iter().any(|candidate| candidate == surface))
        .unwrap_or(false)
}

pub fn surface_command_ids(surface: &str) -> Vec<String> {
    let mut latest: BTreeMap<&str, &CommandSpec> = BTreeMap::new();
    for command in &registry().commands {
        if !command.surfaces.iter().any(|candidate| candidate == surface) {
            continue;
        }
        match latest.get(command.id.as_str()) {
            Some(existing) if existing.version >= command.version => {}
            _ => {
                latest.insert(command.id.as_str(), command);
            }
        }
    }
    latest.keys().map(|id| (*id).to_string()).collect()
}


pub fn compact_list(
    surface: Option<&str>,
    prefix: Option<&str>,
    limit: usize,
) -> Value {
    let mut latest: BTreeMap<&str, &CommandSpec> = BTreeMap::new();
    for command in &registry().commands {
        if let Some(surface) = surface {
            if !command.surfaces.iter().any(|candidate| candidate == surface) {
                continue;
            }
        }
        if let Some(prefix) = prefix {
            if !command.id.starts_with(prefix) {
                continue;
            }
        }
        match latest.get(command.id.as_str()) {
            Some(existing) if existing.version >= command.version => {}
            _ => {
                latest.insert(command.id.as_str(), command);
            }
        }
    }

    let commands: Vec<Value> = latest
        .values()
        .take(limit)
        .map(|command| {
            json!({
                "id": command.id,
                "version": command.version,
                "summary": command.summary,
                "effect_class": command.effect_class,
                "permission": command.permission,
                "stability": command.stability
            })
        })
        .collect();

    json!({
        "registry_format": registry().registry_format,
        "commands": commands
    })
}

pub fn describe(id: &str, version: Option<u32>) -> Result<Value, ResolveError> {
    let command = resolve_command(id, version)?;
    Ok(json!({
        "registry_format": registry().registry_format,
        "common_errors": registry().common_errors,
        "command": command
    }))
}


pub fn render_cli_catalog() -> String {
    let mut lines = Vec::new();
    for id in surface_command_ids("cli") {
        if let Ok(command) = resolve_command(&id, None) {
            lines.push(format!(
                "{}@{}  {}",
                command.id, command.version, command.summary
            ));
        }
    }
    lines.join("\n")
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CompatibilityReport {
    pub backward_compatible: bool,
    pub reasons: Vec<String>,
}

pub fn compare_object_schema_backward(
    old: &Value,
    new: &Value,
) -> CompatibilityReport {
    let mut reasons = Vec::new();
    let Some(old_object) = old.as_object() else {
        return CompatibilityReport {
            backward_compatible: old == new,
            reasons: if old == new {
                Vec::new()
            } else {
                vec!["non-object schema changed".to_string()]
            },
        };
    };
    let Some(new_object) = new.as_object() else {
        return CompatibilityReport {
            backward_compatible: false,
            reasons: vec!["object schema changed to non-object schema".to_string()],
        };
    };

    let old_required: BTreeSet<&str> = old_object
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let new_required: BTreeSet<&str> = new_object
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();

    for added in new_required.difference(&old_required) {
        reasons.push(format!("new required property {added} rejects older payloads"));
    }

    let old_properties = old_object
        .get("properties")
        .and_then(Value::as_object);
    let new_properties = new_object
        .get("properties")
        .and_then(Value::as_object);
    if let Some(old_properties) = old_properties {
        for (name, old_schema) in old_properties {
            match new_properties.and_then(|properties| properties.get(name)) {
                Some(new_schema) if new_schema == old_schema => {}
                Some(_) => reasons.push(format!(
                    "existing property {name} changed schema"
                )),
                None => {
                    if new_object
                        .get("additionalProperties")
                        .and_then(Value::as_bool)
                        == Some(false)
                    {
                        reasons.push(format!(
                            "existing property {name} is no longer accepted"
                        ));
                    }
                }
            }
        }
    }

    if old_object
        .get("additionalProperties")
        .and_then(Value::as_bool)
        != Some(false)
        && new_object
            .get("additionalProperties")
            .and_then(Value::as_bool)
            == Some(false)
    {
        reasons.push(
            "new schema forbids additional properties previously accepted".to_string(),
        );
    }

    CompatibilityReport {
        backward_compatible: reasons.is_empty(),
        reasons,
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_is_valid() {
        validate_embedded_registry().expect("registry must validate");
        assert_eq!(registry().commands.len(), 24);
        assert!(registry()
            .common_errors
            .iter()
            .any(|code| code == "PERMISSION_DENIED"));
        assert_eq!(registry().registry_format, 1);
    }

    #[test]
    fn reserved_and_deprecated_ids_cannot_be_reused() {
        let mut candidate = registry().clone();
        candidate.reserved_command_ids.push("system.status".to_string());
        let error = validate_registry(&candidate).expect_err("reserved reuse must fail");
        assert_eq!(error.code, "REGISTRY_ID_REUSE");

        let mut candidate = registry().clone();
        candidate.deprecated_command_ids.push("system.status".to_string());
        let error = validate_registry(&candidate).expect_err("deprecated reuse must fail");
        assert_eq!(error.code, "REGISTRY_ID_REUSE");
    }

    #[test]
    fn project_register_arguments_are_deterministically_validated() {
        let command = resolve_command("project.register", Some(1)).unwrap();
        validate_value(
            &command.arguments_schema,
            &json!({
                "name": "Fixture",
                "root_uri": "file:///fixture"
            }),
        )
        .expect("valid args");

        let missing = validate_value(
            &command.arguments_schema,
            &json!({ "root_uri": "file:///fixture" }),
        )
        .expect_err("missing name must fail");
        assert!(missing.message.contains("missing required property name"));

        let extra = validate_value(
            &command.arguments_schema,
            &json!({
                "name": "Fixture",
                "root_uri": "file:///fixture",
                "surprise": true
            }),
        )
        .expect_err("unknown property must fail");
        assert!(extra.message.contains("additional property"));
    }

    #[test]
    fn additive_optional_property_is_backward_compatible() {
        let old = json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" }
            },
            "additionalProperties": false
        });
        let new = json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" },
                "note": { "type": "string" }
            },
            "additionalProperties": false
        });
        let report = compare_object_schema_backward(&old, &new);
        assert!(report.backward_compatible);
        assert!(report.reasons.is_empty());
    }

    #[test]
    fn adding_required_property_is_breaking() {
        let old = json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "additionalProperties": false
        });

        let new = json!({
            "type": "object",
            "required": ["name"],
            "properties": { "name": { "type": "string" } },
            "additionalProperties": false
        });
        let report = compare_object_schema_backward(&old, &new);
        assert!(!report.backward_compatible);
        assert!(report.reasons[0].contains("new required property name"));
    }

    #[test]
    fn incompatible_command_version_is_explicit() {
        match resolve_command("system.status", Some(999)) {
            Err(ResolveError::VersionIncompatible { supported }) => {
                assert_eq!(supported, vec![1]);
            }
            other => panic!("unexpected resolution: {other:?}"),
        }
    }

    #[test]
    fn compact_surface_metadata_comes_from_registry() {
        let dashboard = compact_list(Some("dashboard"), None, 200);
        let commands = dashboard["commands"].as_array().unwrap();
        assert!(commands.iter().any(|value| value["id"] == "system.status"));
        assert!(commands.iter().any(|value| value["id"] == "registry.list"));
        assert!(!commands.iter().any(|value| value["id"] == "system.shutdown"));

        let ai = compact_list(Some("ai"), Some("result."), 200);
        assert_eq!(ai["commands"].as_array().unwrap().len(), 2);
    }
}
