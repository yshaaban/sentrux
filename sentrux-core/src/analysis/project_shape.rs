//! Generic repo-archetype and onboarding-shape detection.
//!
//! This is intentionally heuristic and evidence-first. It should help v2 adapt
//! to common TypeScript repo families without hardcoding repo names.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[path = "project_shape/boundaries.rs"]
mod boundaries;
#[path = "project_shape/detect.rs"]
mod detect;
#[path = "project_shape/render.rs"]
mod render;
#[cfg(test)]
#[path = "project_shape/tests.rs"]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProjectArchetypeMatch {
    pub id: String,
    pub confidence: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct BoundaryRootSuggestion {
    pub kind: String,
    pub root: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModuleContractSuggestion {
    pub id: String,
    pub root: String,
    pub public_api: Vec<String>,
    pub nested_public_api: Vec<String>,
    pub confidence: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProjectShapeReport {
    pub configured_archetypes: Vec<String>,
    pub detected_archetypes: Vec<ProjectArchetypeMatch>,
    pub effective_archetypes: Vec<String>,
    pub primary_archetype: Option<String>,
    pub capabilities: Vec<String>,
    pub boundary_roots: Vec<BoundaryRootSuggestion>,
    pub module_contracts: Vec<ModuleContractSuggestion>,
}

pub fn detect_project_shape(
    root: Option<&Path>,
    file_paths: &[String],
    workspace_files: &[String],
    configured_archetypes: &[String],
) -> ProjectShapeReport {
    detect::detect_project_shape(root, file_paths, workspace_files, configured_archetypes)
}

pub fn render_starter_rules(
    shape: &ProjectShapeReport,
    primary_language: Option<&str>,
    existing_excludes: &[String],
) -> String {
    render::render_starter_rules(shape, primary_language, existing_excludes)
}

pub fn render_working_rules(
    shape: &ProjectShapeReport,
    primary_language: Option<&str>,
    existing_excludes: &[String],
) -> String {
    render::render_working_rules(shape, primary_language, existing_excludes)
}
