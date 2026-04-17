use super::obligations_contract::summarize_contract_missing_sites;
use super::{
    build_obligation_findings, build_obligations, obligation_score_0_10000, ObligationScope,
};
use crate::analysis::semantic::{
    ClosedDomain, ExhaustivenessProofKind, ExhaustivenessSite, ExhaustivenessSiteKind,
    ProjectModel, ReadFact, SemanticCapability, SemanticFileFact, SemanticSnapshot, SymbolFact,
};
use crate::metrics::rules::RulesConfig;
use crate::metrics::v2::{
    ObligationConfidence, ObligationOrigin, ObligationReport, ObligationSite, ObligationTrustTier,
};
use std::collections::BTreeSet;

#[path = "obligations_contract_tests.rs"]
mod contract;
#[path = "obligations_domain_tests.rs"]
mod domain;
