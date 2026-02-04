//! Graph traversal and drift detection for Lattice.
//!
//! Linked requirements: REQ-CORE-003, REQ-CORE-005

use crate::storage::load_all_nodes;
use crate::types::{LatticeNode, NodeIndex};
use std::path::Path;

/// Severity of version drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftSeverity {
    Patch,
    Minor,
    Major,
}

/// A single drift item.
#[derive(Debug, Clone)]
pub struct DriftItem {
    pub target_id: String,
    pub bound_version: String,
    pub current_version: String,
    pub severity: DriftSeverity,
}

/// A drift report for a single node.
#[derive(Debug, Clone)]
pub struct DriftReport {
    pub node_id: String,
    pub node_type: String,
    pub drift_items: Vec<DriftItem>,
}

/// Build an index of all nodes keyed by ID.
pub fn build_node_index(root: &Path) -> Result<NodeIndex, crate::storage::StorageError> {
    let nodes = load_all_nodes(root)?;
    let mut index = NodeIndex::new();
    for node in nodes {
        index.insert(node.id.clone(), node);
    }
    Ok(index)
}

/// Compare two semantic versions and return the severity of change.
fn compare_versions(old: &str, new: &str) -> Option<DriftSeverity> {
    let parse = |v: &str| -> Option<(u64, u64, u64)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some((
            parts[0].parse().ok()?,
            parts[1].parse().ok()?,
            parts[2].parse().ok()?,
        ))
    };

    let (old_major, old_minor, old_patch) = parse(old)?;
    let (new_major, new_minor, new_patch) = parse(new)?;

    if new_major > old_major {
        Some(DriftSeverity::Major)
    } else if new_minor > old_minor {
        Some(DriftSeverity::Minor)
    } else if new_patch > old_patch {
        Some(DriftSeverity::Patch)
    } else {
        None
    }
}

/// Find all drift in the lattice.
pub fn find_drift(root: &Path) -> Result<Vec<DriftReport>, crate::storage::StorageError> {
    let index = build_node_index(root)?;
    let mut reports = Vec::new();

    for node in index.values() {
        let mut drift_items = Vec::new();

        for edge_ref in node.all_edges() {
            if let Some(target_node) = index.get(&edge_ref.target) {
                let bound_version = edge_ref.version_or_default();
                if let Some(severity) = compare_versions(bound_version, &target_node.version) {
                    drift_items.push(DriftItem {
                        target_id: edge_ref.target.clone(),
                        bound_version: bound_version.to_string(),
                        current_version: target_node.version.clone(),
                        severity,
                    });
                }
            }
        }

        if !drift_items.is_empty() {
            reports.push(DriftReport {
                node_id: node.id.clone(),
                node_type: format!("{:?}", node.node_type).to_lowercase(),
                drift_items,
            });
        }
    }

    Ok(reports)
}

/// Traverse the graph starting from a node.
pub fn traverse_from(
    start_id: &str,
    index: &NodeIndex,
    visited: &mut std::collections::HashSet<String>,
) -> Vec<LatticeNode> {
    let mut result = Vec::new();

    if visited.contains(start_id) {
        return result;
    }
    visited.insert(start_id.to_string());

    if let Some(node) = index.get(start_id) {
        result.push(node.clone());

        for edge_ref in node.all_edges() {
            let children = traverse_from(&edge_ref.target, index, visited);
            result.extend(children);
        }
    }

    result
}
