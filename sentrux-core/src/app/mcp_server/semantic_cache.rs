use crate::analysis::semantic::typescript::TS_BRIDGE_PROTOCOL_VERSION;
use crate::analysis::semantic::{ProjectModel, SemanticSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticCacheIdentity {
    pub project_fingerprint: String,
    pub bridge_protocol_version: String,
    pub git_head: Option<String>,
    pub working_tree_paths: BTreeSet<String>,
    pub working_tree_hashes: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticCacheSource {
    Memory,
    Disk,
    Bridge,
}

impl SemanticCacheSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Disk => "disk",
            Self::Bridge => "bridge",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedSemanticSnapshot {
    identity: SemanticCacheIdentity,
    snapshot: SemanticSnapshot,
}

#[derive(Serialize)]
struct PersistedSemanticSnapshotRef<'a> {
    identity: &'a SemanticCacheIdentity,
    snapshot: &'a SemanticSnapshot,
}

pub fn current_semantic_cache_identity(
    project: &ProjectModel,
    git_head: Option<String>,
    working_tree_paths: BTreeSet<String>,
    working_tree_hashes: BTreeMap<String, u64>,
) -> SemanticCacheIdentity {
    SemanticCacheIdentity {
        project_fingerprint: project.fingerprint.clone(),
        bridge_protocol_version: TS_BRIDGE_PROTOCOL_VERSION.to_string(),
        git_head,
        working_tree_paths,
        working_tree_hashes,
    }
}

pub fn semantic_snapshot_cache_path(root: &Path) -> PathBuf {
    root.join(".sentrux")
        .join("cache")
        .join("v2")
        .join("semantic-snapshot.json")
}

pub fn load_persisted_semantic_snapshot(
    root: &Path,
    identity: &SemanticCacheIdentity,
) -> Result<Option<SemanticSnapshot>, String> {
    let cache_path = semantic_snapshot_cache_path(root);
    if !cache_path.exists() {
        return Ok(None);
    }

    let bytes = std::fs::read(&cache_path)
        .map_err(|error| format!("Failed to read {}: {error}", cache_path.display()))?;
    let persisted = serde_json::from_slice::<PersistedSemanticSnapshot>(&bytes)
        .map_err(|error| format!("Failed to decode {}: {error}", cache_path.display()))?;
    if persisted.identity != *identity {
        return Ok(None);
    }

    Ok(Some(persisted.snapshot))
}

pub fn save_persisted_semantic_snapshot(
    root: &Path,
    identity: &SemanticCacheIdentity,
    snapshot: &SemanticSnapshot,
) -> Result<PathBuf, String> {
    let cache_path = semantic_snapshot_cache_path(root);
    let parent = cache_path
        .parent()
        .ok_or_else(|| format!("Invalid semantic cache path: {}", cache_path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&PersistedSemanticSnapshotRef { identity, snapshot })
        .map_err(|error| format!("Failed to encode semantic cache: {error}"))?;
    std::fs::write(&cache_path, bytes)
        .map_err(|error| format!("Failed to write {}: {error}", cache_path.display()))?;

    Ok(cache_path)
}
