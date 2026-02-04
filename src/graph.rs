//! Graph traversal and drift detection for Lattice.
//!
//! Linked requirements: REQ-CORE-003, REQ-CORE-005, REQ-CLI-007

use crate::storage::load_all_nodes;
use crate::types::{LatticeNode, NodeIndex, Resolution};
use std::collections::{HashMap, HashSet};
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
    visited: &mut HashSet<String>,
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

/// A planned requirement with its dependencies and status.
#[derive(Debug, Clone)]
pub struct PlannedItem {
    pub id: String,
    pub title: String,
    pub resolution: Option<Resolution>,
    pub depends_on: Vec<String>,
    pub blocked_by: Vec<String>, // Unresolved dependencies
    pub order: usize,
}

/// A plan for implementing requirements.
#[derive(Debug, Clone)]
pub struct Plan {
    pub items: Vec<PlannedItem>,
    pub ready: Vec<String>,    // Can be implemented now
    pub blocked: Vec<String>,  // Waiting on dependencies
    pub verified: Vec<String>, // Already done
}

/// Find all upstream dependencies (what this node depends on).
pub fn find_dependencies(node_id: &str, index: &NodeIndex) -> Vec<String> {
    let mut deps = Vec::new();

    let Some(node) = index.get(node_id) else {
        return deps;
    };
    let Some(edges) = &node.edges else {
        return deps;
    };

    if let Some(depends_on) = &edges.depends_on {
        for edge in depends_on {
            deps.push(edge.target.clone());
        }
    }
    if let Some(derives_from) = &edges.derives_from {
        for edge in derives_from {
            // Only include requirements, not theses
            if edge.target.starts_with("REQ-") {
                deps.push(edge.target.clone());
            }
        }
    }

    deps
}

/// Find all downstream dependents (what depends on this node).
pub fn find_dependents(node_id: &str, index: &NodeIndex) -> Vec<String> {
    let mut dependents = Vec::new();

    for (id, node) in index.iter() {
        let dominated = node
            .edges
            .as_ref()
            .and_then(|e| e.depends_on.as_ref())
            .is_some_and(|deps| deps.iter().any(|e| e.target == node_id));

        if dominated {
            dependents.push(id.clone());
        }
    }

    dependents
}

/// Collect all requirements in the dependency tree.
fn collect_all_deps(node_ids: &[String], index: &NodeIndex, collected: &mut HashSet<String>) {
    for id in node_ids {
        if collected.contains(id) {
            continue;
        }
        collected.insert(id.clone());

        let deps = find_dependencies(id, index);
        collect_all_deps(&deps, index, collected);
    }
}

/// Topological sort of requirements based on dependencies.
fn topological_sort(node_ids: &[String], index: &NodeIndex) -> Vec<String> {
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();

    // Initialize
    for id in node_ids {
        in_degree.entry(id.clone()).or_insert(0);
        graph.entry(id.clone()).or_default();
    }

    // Build graph
    for id in node_ids {
        let deps = find_dependencies(id, index);
        for dep in deps {
            if node_ids.contains(&dep) {
                graph.entry(dep.clone()).or_default().push(id.clone());
                *in_degree.entry(id.clone()).or_insert(0) += 1;
            }
        }
    }

    // Kahn's algorithm
    let mut queue: Vec<String> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(id, _)| id.clone())
        .collect();
    queue.sort(); // Deterministic order

    let mut result = Vec::new();
    while let Some(id) = queue.pop() {
        result.push(id.clone());

        if let Some(dependents) = graph.get(&id) {
            for dep in dependents {
                if let Some(deg) = in_degree.get_mut(dep) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push(dep.clone());
                        queue.sort();
                    }
                }
            }
        }
    }

    result
}

/// Generate a plan for implementing requirements.
pub fn generate_plan(requirement_ids: &[String], index: &NodeIndex) -> Plan {
    // Collect all dependencies
    let mut all_ids: HashSet<String> = HashSet::new();
    collect_all_deps(requirement_ids, index, &mut all_ids);

    // Also include the requested requirements
    for id in requirement_ids {
        all_ids.insert(id.clone());
    }

    // Filter to only requirements that exist
    let valid_ids: Vec<String> = all_ids
        .into_iter()
        .filter(|id| index.contains_key(id) && id.starts_with("REQ-"))
        .collect();

    // Topological sort
    let sorted = topological_sort(&valid_ids, index);

    // Build planned items
    let mut items = Vec::new();
    let mut ready = Vec::new();
    let mut blocked = Vec::new();
    let mut verified = Vec::new();

    for (order, id) in sorted.iter().enumerate() {
        if let Some(node) = index.get(id) {
            let deps = find_dependencies(id, index);
            let resolution = node.resolution.as_ref().map(|r| r.status.clone());

            // Find unresolved dependencies
            let blocked_by: Vec<String> = deps
                .iter()
                .filter(|dep_id| {
                    if let Some(dep_node) = index.get(*dep_id) {
                        !matches!(
                            dep_node.resolution.as_ref().map(|r| &r.status),
                            Some(Resolution::Verified)
                        )
                    } else {
                        false
                    }
                })
                .cloned()
                .collect();

            let item = PlannedItem {
                id: id.clone(),
                title: node.title.clone(),
                resolution: resolution.clone(),
                depends_on: deps,
                blocked_by: blocked_by.clone(),
                order,
            };

            // Categorize
            match resolution {
                Some(Resolution::Verified) => verified.push(id.clone()),
                Some(Resolution::Blocked) | Some(Resolution::Deferred) => blocked.push(id.clone()),
                _ => {
                    if blocked_by.is_empty() {
                        ready.push(id.clone());
                    } else {
                        blocked.push(id.clone());
                    }
                }
            }

            items.push(item);
        }
    }

    Plan {
        items,
        ready,
        blocked,
        verified,
    }
}
