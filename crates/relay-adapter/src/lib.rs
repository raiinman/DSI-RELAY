pub mod backend;
pub mod broker;
pub mod error;
pub mod job;
pub mod manifest;
pub mod sandbox;

pub use backend::{
    current_windows_version, select_backend_for, select_release_backend,
    SandboxBackendSelection, WindowsVersion, MEASURED_WINDOWS_BUILDS,
    SANDBOX_BACKEND_MATRIX_VERSION,
};
pub use broker::{AdapterBroker, InvocationOutcome, InvocationProvenance};
pub use error::AdapterError;
pub use job::{JobGuard, JobLimitEvidence};
pub use manifest::*;
pub use sandbox::{
    run_sandboxed_worker, StableSandboxPolicy, StableSandboxRunEvidence,
};
