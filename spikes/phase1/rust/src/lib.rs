pub mod adapter;
pub mod dashboard;
pub mod diagnostics;
pub mod pipe;
pub mod protocol;
pub mod registry;
pub mod sandbox;
pub mod sandbox_backend;
pub mod stable_sandbox;
pub mod security;
pub mod state;
pub mod storage;

pub const RELAY_VERSION: &str = "0.1.0-phase1-rust";
pub const PROTOCOL_MIN: u32 = 1;
pub const PROTOCOL_MAX: u32 = 1;
pub const SCHEMA_VERSION: u32 = 1;

pub const PLATFORM_CAPABILITIES: &[&str] = &[
    "protocol.handshake@1",
    "dashboard.embedded@1",
    "diagnostics.structured-jsonl@1",
    "diagnostics.bounded-detail@1",
    "ipc.named_pipe.explicit_dacl@1",
    "registry.json-schema-2020-12-subset@1",
];

pub fn capabilities() -> Vec<String> {
    let mut values = registry::capability_ids();
    values.extend(PLATFORM_CAPABILITIES.iter().map(|value| (*value).to_string()));
    values.sort();
    values.dedup();
    values
}
