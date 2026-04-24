//! Core semantic IR types for the v2 semantic substrate.

use crate::analysis::project_shape::ProjectArchetypeMatch;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ProjectModel {
    pub root: String,
    pub tsconfig_paths: Vec<String>,
    pub workspace_files: Vec<String>,
    pub primary_language: Option<String>,
    pub fingerprint: String,
    #[serde(default)]
    pub repo_archetype: Option<String>,
    #[serde(default)]
    pub detected_archetypes: Vec<ProjectArchetypeMatch>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SemanticSnapshot {
    pub project: ProjectModel,
    pub analyzed_files: usize,
    pub capabilities: Vec<SemanticCapability>,
    pub files: Vec<SemanticFileFact>,
    pub symbols: Vec<SymbolFact>,
    pub reads: Vec<ReadFact>,
    pub writes: Vec<WriteFact>,
    pub closed_domains: Vec<ClosedDomain>,
    pub closed_domain_sites: Vec<ExhaustivenessSite>,
    #[serde(default)]
    pub transition_sites: Vec<TransitionSite>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SemanticCapability {
    Symbols,
    References,
    Reads,
    ClosedDomains,
    ClosedDomainSites,
    Writes,
    TransitionSites,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExhaustivenessSiteKind {
    #[default]
    Switch,
    Record,
    Satisfies,
    IfElse,
}

impl ExhaustivenessSiteKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Switch => "switch",
            Self::Record => "record",
            Self::Satisfies => "satisfies",
            Self::IfElse => "if_else",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum ExhaustivenessProofKind {
    #[serde(rename = "switch")]
    #[default]
    Switch,
    #[serde(rename = "assertNever")]
    AssertNever,
    #[serde(rename = "Record")]
    Record,
    #[serde(rename = "satisfies")]
    Satisfies,
    #[serde(rename = "if_else")]
    IfElse,
}

impl ExhaustivenessProofKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Switch => "switch",
            Self::AssertNever => "assertNever",
            Self::Record => "Record",
            Self::Satisfies => "satisfies",
            Self::IfElse => "if_else",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExhaustivenessFallbackKind {
    #[default]
    None,
    Null,
    Undefined,
    GenericString,
    IdentityTransform,
    EmptyArray,
    EmptyObject,
    AssertThrow,
    Other,
}

impl ExhaustivenessFallbackKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Null => "null",
            Self::Undefined => "undefined",
            Self::GenericString => "generic_string",
            Self::IdentityTransform => "identity_transform",
            Self::EmptyArray => "empty_array",
            Self::EmptyObject => "empty_object",
            Self::AssertThrow => "assert_throw",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExhaustivenessSiteSemanticRole {
    Label,
    Target,
    Status,
    Render,
    Handler,
    Policy,
    Serialization,
    Transform,
    #[default]
    Unknown,
}

impl ExhaustivenessSiteSemanticRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Label => "label",
            Self::Target => "target",
            Self::Status => "status",
            Self::Render => "render",
            Self::Handler => "handler",
            Self::Policy => "policy",
            Self::Serialization => "serialization",
            Self::Transform => "transform",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    RecordEntry,
    #[default]
    SwitchCase,
    IfBranch,
    IfElse,
}

impl TransitionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RecordEntry => "record_entry",
            Self::SwitchCase => "switch_case",
            Self::IfBranch => "if_branch",
            Self::IfElse => "if_else",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SemanticFileFact {
    pub path: String,
    pub symbol_count: usize,
    pub write_count: usize,
    pub closed_domain_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SymbolFact {
    pub id: String,
    pub path: String,
    pub name: String,
    pub kind: String,
    pub line: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct WriteFact {
    pub path: String,
    pub symbol_name: String,
    pub write_kind: String,
    pub line: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ReadFact {
    pub path: String,
    pub symbol_name: String,
    pub read_kind: String,
    pub line: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ClosedDomain {
    pub path: String,
    pub symbol_name: String,
    pub variants: Vec<String>,
    pub line: u32,
    #[serde(default)]
    pub defining_file: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ExhaustivenessSite {
    pub path: String,
    pub domain_symbol_name: String,
    #[serde(default)]
    pub defining_file: Option<String>,
    pub site_kind: ExhaustivenessSiteKind,
    pub proof_kind: ExhaustivenessProofKind,
    pub covered_variants: Vec<String>,
    pub line: u32,
    #[serde(default)]
    pub fallback_kind: ExhaustivenessFallbackKind,
    #[serde(default)]
    pub site_expression: Option<String>,
    #[serde(default)]
    pub site_semantic_role: ExhaustivenessSiteSemanticRole,
    #[serde(default)]
    pub site_confidence: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct TransitionSite {
    pub path: String,
    pub domain_symbol_name: String,
    pub group_id: String,
    pub transition_kind: TransitionKind,
    pub source_variant: Option<String>,
    pub target_variants: Vec<String>,
    pub line: u32,
}
