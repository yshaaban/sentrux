//! Core semantic IR types for the v2 semantic substrate.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ProjectModel {
    pub root: String,
    pub tsconfig_paths: Vec<String>,
    pub workspace_files: Vec<String>,
    pub primary_language: Option<String>,
    pub fingerprint: String,
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
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ExhaustivenessSite {
    pub path: String,
    pub domain_symbol_name: String,
    pub site_kind: String,
    pub proof_kind: String,
    pub covered_variants: Vec<String>,
    pub line: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct TransitionSite {
    pub path: String,
    pub domain_symbol_name: String,
    pub group_id: String,
    pub transition_kind: String,
    pub source_variant: Option<String>,
    pub target_variants: Vec<String>,
    pub line: u32,
}
