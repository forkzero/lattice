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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    /// Helper to build a minimal requirement node.
    fn req(
        id: &str,
        version: &str,
        edges: Option<Edges>,
        resolution: Option<Resolution>,
    ) -> LatticeNode {
        LatticeNode {
            id: id.to_string(),
            node_type: NodeType::Requirement,
            title: format!("Test {id}"),
            body: String::new(),
            status: Status::Active,
            version: version.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            created_by: "test".to_string(),
            requested_by: None,
            priority: Some(Priority::P1),
            category: None,
            tags: None,
            acceptance: None,
            visibility: None,
            resolution: resolution.map(|s| ResolutionInfo {
                status: s,
                reason: None,
                resolved_at: "2026-01-01T00:00:00Z".to_string(),
                resolved_by: "test".to_string(),
            }),
            meta: None,
            edges,
        }
    }

    fn edge_ref(target: &str, version: &str) -> EdgeReference {
        EdgeReference {
            target: target.to_string(),
            version: Some(version.to_string()),
            rationale: None,
        }
    }

    fn depends_on(targets: &[(&str, &str)]) -> Option<Edges> {
        Some(Edges {
            depends_on: Some(targets.iter().map(|(t, v)| edge_ref(t, v)).collect()),
            ..Default::default()
        })
    }

    // --- compare_versions ---

    #[test]
    fn test_compare_versions_major() {
        assert_eq!(
            compare_versions("1.0.0", "2.0.0"),
            Some(DriftSeverity::Major)
        );
    }

    #[test]
    fn test_compare_versions_minor() {
        assert_eq!(
            compare_versions("1.0.0", "1.1.0"),
            Some(DriftSeverity::Minor)
        );
    }

    #[test]
    fn test_compare_versions_patch() {
        assert_eq!(
            compare_versions("1.0.0", "1.0.1"),
            Some(DriftSeverity::Patch)
        );
    }

    #[test]
    fn test_compare_versions_no_drift() {
        assert_eq!(compare_versions("1.0.0", "1.0.0"), None);
    }

    #[test]
    fn test_compare_versions_invalid() {
        assert_eq!(compare_versions("1.0", "1.0.0"), None);
        assert_eq!(compare_versions("abc", "1.0.0"), None);
    }

    // --- find_dependencies ---

    #[test]
    fn test_find_dependencies_returns_depends_on_targets() {
        let mut index = NodeIndex::new();
        index.insert(
            "REQ-A".into(),
            req(
                "REQ-A",
                "1.0.0",
                depends_on(&[("REQ-B", "1.0.0"), ("REQ-C", "1.0.0")]),
                None,
            ),
        );
        index.insert("REQ-B".into(), req("REQ-B", "1.0.0", None, None));
        index.insert("REQ-C".into(), req("REQ-C", "1.0.0", None, None));

        let deps = find_dependencies("REQ-A", &index);
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&"REQ-B".to_string()));
        assert!(deps.contains(&"REQ-C".to_string()));
    }

    #[test]
    fn test_find_dependencies_excludes_theses() {
        let mut index = NodeIndex::new();
        let edges = Some(Edges {
            derives_from: Some(vec![
                edge_ref("THX-FOO", "1.0.0"),
                edge_ref("REQ-B", "1.0.0"),
            ]),
            ..Default::default()
        });
        index.insert("REQ-A".into(), req("REQ-A", "1.0.0", edges, None));
        index.insert("REQ-B".into(), req("REQ-B", "1.0.0", None, None));

        let deps = find_dependencies("REQ-A", &index);
        assert_eq!(deps, vec!["REQ-B".to_string()]);
    }

    #[test]
    fn test_find_dependencies_missing_node() {
        let index = NodeIndex::new();
        assert!(find_dependencies("REQ-MISSING", &index).is_empty());
    }

    // --- find_dependents ---

    #[test]
    fn test_find_dependents() {
        let mut index = NodeIndex::new();
        index.insert("REQ-A".into(), req("REQ-A", "1.0.0", None, None));
        index.insert(
            "REQ-B".into(),
            req("REQ-B", "1.0.0", depends_on(&[("REQ-A", "1.0.0")]), None),
        );
        index.insert(
            "REQ-C".into(),
            req("REQ-C", "1.0.0", depends_on(&[("REQ-A", "1.0.0")]), None),
        );

        let dependents = find_dependents("REQ-A", &index);
        assert_eq!(dependents.len(), 2);
        assert!(dependents.contains(&"REQ-B".to_string()));
        assert!(dependents.contains(&"REQ-C".to_string()));
    }

    #[test]
    fn test_find_dependents_none() {
        let mut index = NodeIndex::new();
        index.insert("REQ-A".into(), req("REQ-A", "1.0.0", None, None));

        assert!(find_dependents("REQ-A", &index).is_empty());
    }

    // --- traverse_from ---

    #[test]
    fn test_traverse_from_follows_edges() {
        let mut index = NodeIndex::new();
        index.insert(
            "REQ-A".into(),
            req("REQ-A", "1.0.0", depends_on(&[("REQ-B", "1.0.0")]), None),
        );
        index.insert(
            "REQ-B".into(),
            req("REQ-B", "1.0.0", depends_on(&[("REQ-C", "1.0.0")]), None),
        );
        index.insert("REQ-C".into(), req("REQ-C", "1.0.0", None, None));

        let mut visited = HashSet::new();
        let result = traverse_from("REQ-A", &index, &mut visited);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].id, "REQ-A");
    }

    #[test]
    fn test_traverse_from_handles_cycles() {
        let mut index = NodeIndex::new();
        index.insert(
            "REQ-A".into(),
            req("REQ-A", "1.0.0", depends_on(&[("REQ-B", "1.0.0")]), None),
        );
        index.insert(
            "REQ-B".into(),
            req("REQ-B", "1.0.0", depends_on(&[("REQ-A", "1.0.0")]), None),
        );

        let mut visited = HashSet::new();
        let result = traverse_from("REQ-A", &index, &mut visited);

        // Should visit each node exactly once despite cycle
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_traverse_from_missing_start() {
        let index = NodeIndex::new();
        let mut visited = HashSet::new();
        let result = traverse_from("REQ-MISSING", &index, &mut visited);
        assert!(result.is_empty());
    }

    // --- topological_sort ---

    #[test]
    fn test_topological_sort_linear_chain() {
        let mut index = NodeIndex::new();
        index.insert("REQ-A".into(), req("REQ-A", "1.0.0", None, None));
        index.insert(
            "REQ-B".into(),
            req("REQ-B", "1.0.0", depends_on(&[("REQ-A", "1.0.0")]), None),
        );
        index.insert(
            "REQ-C".into(),
            req("REQ-C", "1.0.0", depends_on(&[("REQ-B", "1.0.0")]), None),
        );

        let ids: Vec<String> = vec!["REQ-A".into(), "REQ-B".into(), "REQ-C".into()];
        let sorted = topological_sort(&ids, &index);

        let pos_a = sorted.iter().position(|x| x == "REQ-A").unwrap();
        let pos_b = sorted.iter().position(|x| x == "REQ-B").unwrap();
        let pos_c = sorted.iter().position(|x| x == "REQ-C").unwrap();

        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn test_topological_sort_diamond() {
        // A -> B, A -> C, B -> D, C -> D
        let mut index = NodeIndex::new();
        index.insert("REQ-A".into(), req("REQ-A", "1.0.0", None, None));
        index.insert(
            "REQ-B".into(),
            req("REQ-B", "1.0.0", depends_on(&[("REQ-A", "1.0.0")]), None),
        );
        index.insert(
            "REQ-C".into(),
            req("REQ-C", "1.0.0", depends_on(&[("REQ-A", "1.0.0")]), None),
        );
        index.insert(
            "REQ-D".into(),
            req(
                "REQ-D",
                "1.0.0",
                depends_on(&[("REQ-B", "1.0.0"), ("REQ-C", "1.0.0")]),
                None,
            ),
        );

        let ids: Vec<String> = vec![
            "REQ-A".into(),
            "REQ-B".into(),
            "REQ-C".into(),
            "REQ-D".into(),
        ];
        let sorted = topological_sort(&ids, &index);

        let pos_a = sorted.iter().position(|x| x == "REQ-A").unwrap();
        let pos_b = sorted.iter().position(|x| x == "REQ-B").unwrap();
        let pos_c = sorted.iter().position(|x| x == "REQ-C").unwrap();
        let pos_d = sorted.iter().position(|x| x == "REQ-D").unwrap();

        assert!(pos_a < pos_b);
        assert!(pos_a < pos_c);
        assert!(pos_b < pos_d);
        assert!(pos_c < pos_d);
    }

    #[test]
    fn test_topological_sort_independent_nodes() {
        let mut index = NodeIndex::new();
        index.insert("REQ-A".into(), req("REQ-A", "1.0.0", None, None));
        index.insert("REQ-B".into(), req("REQ-B", "1.0.0", None, None));

        let ids: Vec<String> = vec!["REQ-A".into(), "REQ-B".into()];
        let sorted = topological_sort(&ids, &index);

        assert_eq!(sorted.len(), 2);
        assert!(sorted.contains(&"REQ-A".to_string()));
        assert!(sorted.contains(&"REQ-B".to_string()));
    }

    // --- generate_plan ---

    #[test]
    fn test_generate_plan_categorizes_items() {
        let mut index = NodeIndex::new();
        index.insert(
            "REQ-A".into(),
            req("REQ-A", "1.0.0", None, Some(Resolution::Verified)),
        );
        index.insert(
            "REQ-B".into(),
            req("REQ-B", "1.0.0", depends_on(&[("REQ-A", "1.0.0")]), None),
        );
        index.insert(
            "REQ-C".into(),
            req("REQ-C", "1.0.0", depends_on(&[("REQ-B", "1.0.0")]), None),
        );

        let plan = generate_plan(&["REQ-C".into()], &index);

        assert!(plan.verified.contains(&"REQ-A".to_string()));
        assert!(plan.ready.contains(&"REQ-B".to_string()));
        assert!(plan.blocked.contains(&"REQ-C".to_string()));
    }

    #[test]
    fn test_generate_plan_all_deps_verified_means_ready() {
        let mut index = NodeIndex::new();
        index.insert(
            "REQ-A".into(),
            req("REQ-A", "1.0.0", None, Some(Resolution::Verified)),
        );
        index.insert(
            "REQ-B".into(),
            req("REQ-B", "1.0.0", depends_on(&[("REQ-A", "1.0.0")]), None),
        );

        let plan = generate_plan(&["REQ-B".into()], &index);

        assert!(plan.ready.contains(&"REQ-B".to_string()));
        assert!(plan.blocked.is_empty());
    }

    #[test]
    fn test_generate_plan_blocked_resolution() {
        let mut index = NodeIndex::new();
        index.insert(
            "REQ-A".into(),
            req("REQ-A", "1.0.0", None, Some(Resolution::Blocked)),
        );

        let plan = generate_plan(&["REQ-A".into()], &index);

        assert!(plan.blocked.contains(&"REQ-A".to_string()));
        assert!(plan.ready.is_empty());
    }

    #[test]
    fn test_generate_plan_empty_input() {
        let index = NodeIndex::new();
        let plan = generate_plan(&[], &index);

        assert!(plan.items.is_empty());
        assert!(plan.ready.is_empty());
        assert!(plan.blocked.is_empty());
        assert!(plan.verified.is_empty());
    }

    // --- find_drift (file-based, using tempdir) ---

    #[test]
    fn test_find_drift_detects_version_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let req_dir = root.join(".lattice").join("requirements");
        std::fs::create_dir_all(&req_dir).unwrap();

        // REQ-A at version 2.0.0
        std::fs::write(
            req_dir.join("req-a.yaml"),
            "id: REQ-A\ntype: requirement\ntitle: A\nbody: test\nstatus: active\nversion: '2.0.0'\ncreated_at: '2026-01-01'\ncreated_by: test\n",
        ).unwrap();

        // REQ-B depends on REQ-A at version 1.0.0 (stale)
        std::fs::write(
            req_dir.join("req-b.yaml"),
            "id: REQ-B\ntype: requirement\ntitle: B\nbody: test\nstatus: active\nversion: '1.0.0'\ncreated_at: '2026-01-01'\ncreated_by: test\nedges:\n  depends_on:\n    - target: REQ-A\n      version: '1.0.0'\n",
        ).unwrap();

        let reports = find_drift(root).unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].node_id, "REQ-B");
        assert_eq!(reports[0].drift_items[0].target_id, "REQ-A");
        assert_eq!(reports[0].drift_items[0].severity, DriftSeverity::Major);
    }

    #[test]
    fn test_find_drift_clean_when_versions_match() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let req_dir = root.join(".lattice").join("requirements");
        std::fs::create_dir_all(&req_dir).unwrap();

        std::fs::write(
            req_dir.join("req-a.yaml"),
            "id: REQ-A\ntype: requirement\ntitle: A\nbody: test\nstatus: active\nversion: '1.0.0'\ncreated_at: '2026-01-01'\ncreated_by: test\n",
        ).unwrap();

        std::fs::write(
            req_dir.join("req-b.yaml"),
            "id: REQ-B\ntype: requirement\ntitle: B\nbody: test\nstatus: active\nversion: '1.0.0'\ncreated_at: '2026-01-01'\ncreated_by: test\nedges:\n  depends_on:\n    - target: REQ-A\n      version: '1.0.0'\n",
        ).unwrap();

        let reports = find_drift(root).unwrap();
        assert!(reports.is_empty());
    }
}
