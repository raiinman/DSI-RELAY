pub mod context;
pub mod diagnostics;
pub mod indexing;
pub mod policy;
pub mod planner;
pub mod service;
pub mod storage;

pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");
