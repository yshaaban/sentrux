//! Semantic analysis substrate for v2.
//!
//! This layer is intentionally separate from the scanner and graph builder:
//! it owns typed, symbol-level facts that require language-specific frontends.

pub mod project;
pub mod types;
pub mod typescript;

pub use project::discover_project;
pub use types::{
    ClosedDomain, ExhaustivenessSite, ProjectModel, ReadFact, SemanticCapability, SemanticFileFact,
    SemanticSnapshot, SymbolFact, WriteFact,
};
