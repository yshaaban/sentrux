//! V2 semantic findings built on explicit rules and semantic facts.

mod authority_access;
mod boundary;
mod boundary_bypass;
mod clones;
mod concentration;
mod concept_match;
mod obligations;
mod parity;
mod parity_support;
mod state;
mod state_support;
mod structural;
mod test_coverage;

use crate::analysis::semantic::{ReadFact, SemanticSnapshot, WriteFact};
use crate::metrics::rules::{ConceptRule, RulesConfig};
use crate::metrics::testgap::is_test_file;
use crate::string_enum::impl_str_enum;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub use authority_access::{
    build_authority_and_access_findings, build_authority_and_access_findings_with_snapshot,
};
pub use boundary::build_zero_config_boundary_findings;
pub use clones::{
    build_clone_drift_findings, build_clone_drift_report, build_clone_remediation_hints,
    CloneDriftFinding, CloneDriftInstance, CloneDriftReport, CloneFamilySummary,
    CloneRemediationHint, RemediationPriority,
};
pub use concentration::{
    build_concentration_findings, build_concentration_reports, ConcentrationBuildResult,
    ConcentrationFinding, ConcentrationHistory, ConcentrationReport,
};
pub(crate) use concept_match::{
    concept_targets, relevant_production_writes, relevant_reads, relevant_writes,
    symbol_from_scoped_path, symbol_matches_targets,
};
pub use obligations::{
    build_obligation_findings, build_obligations, changed_concept_ids_from_files,
    changed_concepts_from_obligations, obligation_score_0_10000, ObligationConfidence,
    ObligationOrigin, ObligationReport, ObligationScope, ObligationSite, ObligationTrustTier,
};
pub use parity::{
    build_parity_findings, build_parity_reports, parity_score_0_10000, ContractParityReport,
    ParityBuildResult, ParityCell, ParityScope,
};
pub use state::{
    build_state_integrity_findings, build_state_integrity_reports,
    changed_state_model_ids_from_files, state_integrity_score_0_10000, StateIntegrityReport,
    StateScope,
};
pub use structural::{
    build_structural_debt_reports, build_structural_debt_reports_with_root, StructuralDebtMetrics,
    StructuralDebtReport, StructuralLeverageClass, StructuralPresentationClass,
    StructuralSignalClass, StructuralTrustTier,
};
pub use test_coverage::build_missing_test_findings;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingSeverity {
    Low,
    Medium,
    High,
}

impl FindingSeverity {
    pub const fn priority(self) -> u8 {
        match self {
            Self::High => 3,
            Self::Medium => 2,
            Self::Low => 1,
        }
    }
}

impl_str_enum!(FindingSeverity {
    Low => "low",
    Medium => "medium",
    High => "high",
});

impl Default for FindingSeverity {
    fn default() -> Self {
        Self::Low
    }
}

impl std::fmt::Display for FindingSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFinding {
    pub kind: String,
    pub severity: FindingSeverity,
    pub concept_id: String,
    pub summary: String,
    pub files: Vec<String>,
    pub evidence: Vec<String>,
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
