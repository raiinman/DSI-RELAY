pub mod dashboard;
pub mod pipe;
pub mod protocol;
pub mod security;
pub mod state;

pub const RELAY_VERSION: &str = "0.1.0-phase1-rust";
pub const PROTOCOL_MIN: u32 = 1;
pub const PROTOCOL_MAX: u32 = 1;
pub const SCHEMA_VERSION: u32 = 1;

pub const CAPABILITIES: &[&str] = &[
    "protocol.handshake@1",
    "system.status@1",
    "system.doctor@1",
    "system.echo@1",
    "system.shutdown@1",
    "dashboard.embedded@1",
    "ipc.named_pipe.explicit_dacl@1",
];
