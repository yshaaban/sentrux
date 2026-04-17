use super::boundary_bypass::authoritative_import_bypass_findings;
use super::concept_match::{
    pattern_list_matches, relevant_production_reads, relevant_production_writes,
    sorted_deduped_strings,
};
use super::*;
use crate::core::snapshot::Snapshot;

pub fn build_authority_and_access_findings(
    config: &RulesConfig,
    semantic: &SemanticSnapshot,
) -> Vec<SemanticFinding> {
    build_authority_and_access_findings_with_snapshot(config, semantic, None)
}

pub fn build_authority_and_access_findings_with_snapshot(
    config: &RulesConfig,
    semantic: &SemanticSnapshot,
    snapshot: Option<&Snapshot>,
) -> Vec<SemanticFinding> {
    let mut findings = Vec::new();

    for concept in &config.concept {
        findings.extend(multi_writer_findings(concept, semantic));
        findings.extend(writer_policy_findings(concept, semantic));
        findings.extend(raw_access_findings(concept, semantic));
        if let Some(snapshot) = snapshot {
            findings.extend(authoritative_import_bypass_findings(
                concept, semantic, snapshot,
            ));
        }
    }

    findings
}

fn multi_writer_findings(
    concept: &ConceptRule,
    semantic: &SemanticSnapshot,
) -> Vec<SemanticFinding> {
    let writes = relevant_production_writes(concept, semantic);
    let writer_files: BTreeSet<String> = writes.iter().map(|write| write.path.clone()).collect();
    if writer_files.len() <= 1 {
        return Vec::new();
    }

    vec![SemanticFinding {
        kind: "multi_writer_concept".to_string(),
        severity: FindingSeverity::High,
        concept_id: concept.id.clone(),
        summary: format!(
            "Concept '{}' is mutated from {} files",
            concept.id,
            writer_files.len()
        ),
        files: writer_files.iter().cloned().collect(),
        evidence: sorted_deduped_strings(writes.iter().map(|write| {
            format!(
                "{}::{} ({})",
                write.path, write.symbol_name, write.write_kind
            )
        })),
    }]
}

fn writer_policy_findings(
    concept: &ConceptRule,
    semantic: &SemanticSnapshot,
) -> Vec<SemanticFinding> {
    let writes = relevant_production_writes(concept, semantic);
    let mut forbidden_writes = BTreeMap::<String, BTreeSet<String>>::new();
    let mut outside_allowlist_writes = BTreeMap::<String, BTreeSet<String>>::new();

    for write in writes {
        let scoped_target = format!("{}::{}", write.path, write.symbol_name);
        if pattern_list_matches(&concept.forbid_writers, &write.path, &write.symbol_name) {
            forbidden_writes
                .entry(write.path.clone())
                .or_default()
                .insert(scoped_target);
            continue;
        }

        if !concept.allowed_writers.is_empty()
            && !pattern_list_matches(&concept.allowed_writers, &write.path, &write.symbol_name)
        {
            outside_allowlist_writes
                .entry(write.path.clone())
                .or_default()
                .insert(scoped_target);
        }
    }

    let forbidden_findings = forbidden_writes
        .into_iter()
        .map(|(path, evidence)| SemanticFinding {
            kind: "forbidden_writer".to_string(),
            severity: FindingSeverity::High,
            concept_id: concept.id.clone(),
            summary: format!(
                "Concept '{}' is written from forbidden location {}",
                concept.id, path
            ),
            files: vec![path],
            evidence: evidence.into_iter().collect(),
        });
    let outside_allowlist_findings =
        outside_allowlist_writes
            .into_iter()
            .map(|(path, evidence)| SemanticFinding {
                kind: "writer_outside_allowlist".to_string(),
                severity: FindingSeverity::High,
                concept_id: concept.id.clone(),
                summary: format!(
                    "Concept '{}' is written outside its allowed writer set at {}",
                    concept.id, path
                ),
                files: vec![path],
                evidence: evidence.into_iter().collect(),
            });

    forbidden_findings
        .chain(outside_allowlist_findings)
        .collect()
}

fn raw_access_findings(concept: &ConceptRule, semantic: &SemanticSnapshot) -> Vec<SemanticFinding> {
    let reads = relevant_production_reads(concept, semantic);
    let mut forbidden_reads = BTreeMap::<String, BTreeSet<String>>::new();
    let preferred_accessors = concept.canonical_accessors.clone();

    for read in reads {
        if !pattern_list_matches(&concept.forbid_raw_reads, &read.path, &read.symbol_name) {
            continue;
        }

        let scoped_target = format!("{}::{}", read.path, read.symbol_name);
        forbidden_reads
            .entry(read.path.clone())
            .or_default()
            .insert(scoped_target);
    }

    forbidden_reads
        .into_iter()
        .map(|(path, evidence)| {
            let mut evidence = evidence.into_iter().collect::<Vec<_>>();
            append_preferred_accessor_evidence(&mut evidence, &preferred_accessors);
            append_canonical_owner_evidence(&mut evidence, concept);

            SemanticFinding {
                kind: "forbidden_raw_read".to_string(),
                severity: FindingSeverity::Medium,
                concept_id: concept.id.clone(),
                summary: format!(
                    "Concept '{}' is read from a forbidden raw access path at {}",
                    concept.id, path
                ),
                files: vec![path],
                evidence,
            }
        })
        .collect()
}

fn append_preferred_accessor_evidence(evidence: &mut Vec<String>, preferred_accessors: &[String]) {
    for accessor in preferred_accessors {
        let accessor_evidence = format!("preferred accessor: {accessor}");
        if !evidence.contains(&accessor_evidence) {
            evidence.push(accessor_evidence);
        }
    }
}

fn append_canonical_owner_evidence(evidence: &mut Vec<String>, concept: &ConceptRule) {
    let Some(owner) = concept
        .anchors
        .first()
        .or_else(|| concept.authoritative_inputs.first())
    else {
        return;
    };

    let owner_evidence = format!("canonical owner: {owner}");
    if !evidence.contains(&owner_evidence) {
        evidence.push(owner_evidence);
    }
}
