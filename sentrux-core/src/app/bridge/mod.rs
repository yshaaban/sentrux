//! TypeScript bridge process management for the v2 semantic substrate.

pub mod supervisor;

pub use supervisor::{BridgeError, TypeScriptBridgeSupervisor};
