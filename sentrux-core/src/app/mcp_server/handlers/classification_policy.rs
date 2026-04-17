use super::signal_policy::{report_leverage_rank, report_presentation_rank};
use super::*;
use crate::string_enum::impl_str_enum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FindingTrustTier {
    Trusted,
    Watchpoint,
    Experimental,
}

impl FindingTrustTier {
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "watchpoint" => Self::Watchpoint,
            "experimental" => Self::Experimental,
            _ => Self::Trusted,
        }
    }
}

impl Default for FindingTrustTier {
    fn default() -> Self {
        Self::Trusted
    }
}

impl_str_enum!(FindingTrustTier {
    Trusted => "trusted",
    Watchpoint => "watchpoint",
    Experimental => "experimental",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FindingPresentationClass {
    StructuralDebt,
    GuardedFacade,
    ToolingDebt,
    HardeningNote,
    Watchpoint,
    Experimental,
}

impl FindingPresentationClass {
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "guarded_facade" => Self::GuardedFacade,
            "tooling_debt" => Self::ToolingDebt,
            "hardening_note" => Self::HardeningNote,
            "watchpoint" => Self::Watchpoint,
            "experimental" => Self::Experimental,
            _ => Self::StructuralDebt,
        }
    }

    pub(crate) fn rank(self) -> usize {
        report_presentation_rank(self.as_str())
    }
}

impl Default for FindingPresentationClass {
    fn default() -> Self {
        Self::StructuralDebt
    }
}

impl_str_enum!(FindingPresentationClass {
    StructuralDebt => "structural_debt",
    GuardedFacade => "guarded_facade",
    ToolingDebt => "tooling_debt",
    HardeningNote => "hardening_note",
    Watchpoint => "watchpoint",
    Experimental => "experimental",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FindingLeverageClass {
    SecondaryCleanup,
    LocalRefactorTarget,
    ArchitectureSignal,
    RegrowthWatchpoint,
    ToolingDebt,
    BoundaryDiscipline,
    HardeningNote,
    Experimental,
}

impl Default for FindingLeverageClass {
    fn default() -> Self {
        Self::SecondaryCleanup
    }
}

impl FindingLeverageClass {
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "local_refactor_target" => Self::LocalRefactorTarget,
            "architecture_signal" => Self::ArchitectureSignal,
            "regrowth_watchpoint" => Self::RegrowthWatchpoint,
            "tooling_debt" => Self::ToolingDebt,
            "boundary_discipline" => Self::BoundaryDiscipline,
            "hardening_note" => Self::HardeningNote,
            "experimental" => Self::Experimental,
            _ => Self::SecondaryCleanup,
        }
    }

    pub(crate) fn rank(self) -> usize {
        report_leverage_rank(self.as_str())
    }
}

impl_str_enum!(FindingLeverageClass {
    SecondaryCleanup => "secondary_cleanup",
    LocalRefactorTarget => "local_refactor_target",
    ArchitectureSignal => "architecture_signal",
    RegrowthWatchpoint => "regrowth_watchpoint",
    ToolingDebt => "tooling_debt",
    BoundaryDiscipline => "boundary_discipline",
    HardeningNote => "hardening_note",
    Experimental => "experimental",
});

pub(super) fn finding_trust_tier(finding: &Value) -> FindingTrustTier {
    finding
        .get("trust_tier")
        .and_then(|value| value.as_str())
        .map(FindingTrustTier::from_str)
        .unwrap_or_else(|| trust_tier_for_kind(finding_kind(finding), FindingTrustTier::Trusted))
}

pub(super) fn role_tags_include(role_tags: &[String], tag: &str) -> bool {
    role_tags.iter().any(|role_tag| role_tag == tag)
}

fn trust_tier_for_kind(kind: &str, default: FindingTrustTier) -> FindingTrustTier {
    match kind {
        "cycle_cluster"
        | "dead_island"
        | "missing_test_coverage"
        | "zero_config_boundary_violation"
        | "clone_propagation_drift"
        | "touched_clone_family" => FindingTrustTier::Watchpoint,
        "dead_private_code_cluster" => FindingTrustTier::Experimental,
        _ => default,
    }
}
