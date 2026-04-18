use super::*;

#[path = "classification_details.rs"]
mod classification_details;
#[path = "classification_leverage.rs"]
mod classification_leverage;
#[path = "classification_policy.rs"]
mod classification_policy;
#[path = "classification_presentation.rs"]
mod classification_presentation;
#[path = "classification_readers.rs"]
mod classification_readers;

pub(crate) use classification_details::{
    build_finding_details, classify_default_surface_role, classify_primary_lane,
    decorate_finding_with_classification, is_experimental_finding, merge_findings,
    partition_experimental_findings, partition_review_surface_experimental_findings,
    severity_of_value,
};
pub(crate) use classification_leverage::backfill_leverage_fields;
use classification_leverage::{
    classify_leverage_class_internal, classify_leverage_reasons_internal, finding_leverage_class,
    finding_leverage_reasons,
};
use classification_policy::{finding_trust_tier, role_tags_include};
pub(crate) use classification_policy::{
    FindingLeverageClass, FindingPresentationClass, FindingTrustTier,
};
pub(crate) use classification_presentation::classify_presentation_class;
use classification_presentation::{
    finding_presentation_class, is_contract_surface_propagation_kind,
};
use classification_readers::finding_string_values;
pub(crate) use classification_readers::{
    combined_other_finding_values, dedupe_strings_preserve_order, finding_concept_id,
    finding_files, finding_kind, finding_payload_map, finding_scope, finding_values,
    serialized_values,
};
