//! Core types and utilities shared across all layers.
//!
//! Contains the canonical data types (FileNode, Snapshot, graph edges),
//! application settings (including visual themes), error types, and
//! utility functions. No layer-specific logic lives here — only shared
//! vocabulary.

pub mod heat;
pub mod path_utils;
pub mod settings;
pub mod snapshot;
pub mod types;
