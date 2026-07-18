//! Milestone-triggered knowledge capture.
//!
//! Implements a *nomination* pipeline: at git milestones (pre-commit, pre-push)
//! agent activity is evaluated against the graph and surfaced as typed,
//! human-reviewable proposals in a staging inbox — never written to the graph
//! directly. The AI notices and proposes; a human commits.
//!
//! Two deterministic building blocks live here:
//!   * a binding-intersection *prefilter* that answers "does this change touch
//!     anything the lattice tracks?" with no model call (REQ-CAPTURE-002), and
//!   * a staging *inbox* held outside `.lattice/` so nominations can accumulate
//!     without mutating the curated graph (REQ-CAPTURE-001, REQ-CAPTURE-005).
//!
//! Linked requirements: REQ-CAPTURE-001, REQ-CAPTURE-002, REQ-CAPTURE-003,
//! REQ-CAPTURE-004, REQ-CAPTURE-005

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::types::{LatticeNode, NodeMeta, NodeType};

/// Staging inbox file. Lives at the repo root, deliberately *outside* `.lattice/`
/// so nominations never masquerade as committed graph nodes (REQ-CAPTURE-001, 005).
pub const INBOX_FILE: &str = "lattice-inbox.yaml";

/// The kind of graph change a nomination proposes. Each maps to a typed lattice
/// operation applied only on explicit `accept`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NominationKind {
    /// Propose a brand-new requirement.
    NewRequirement,
    /// Propose a `reveals_gap_in` edge to an existing node.
    RevealsGapIn,
    /// Propose a `challenges` edge to an existing thesis.
    Challenges,
    /// Propose a `validates` edge to an existing thesis.
    Validates,
}

impl NominationKind {
    /// Parse from a CLI string (accepts kebab or snake case).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().replace('_', "-").as_str() {
            "new-requirement" => Ok(Self::NewRequirement),
            "reveals-gap-in" => Ok(Self::RevealsGapIn),
            "challenges" => Ok(Self::Challenges),
            "validates" => Ok(Self::Validates),
            _ => Err(format!(
                "Invalid nomination kind '{}'. Must be one of: new-requirement, reveals-gap-in, challenges, validates",
                s
            )),
        }
    }

    /// The lattice edge type this kind materializes into, or `None` for
    /// `new-requirement` (which creates a node, not an edge).
    pub fn edge_type(self) -> Option<&'static str> {
        match self {
            Self::NewRequirement => None,
            Self::RevealsGapIn => Some("reveals_gap_in"),
            Self::Challenges => Some("challenges"),
            Self::Validates => Some("validates"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::NewRequirement => "new-requirement",
            Self::RevealsGapIn => "reveals-gap-in",
            Self::Challenges => "challenges",
            Self::Validates => "validates",
        }
    }
}

/// A single staged proposal awaiting human review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Nomination {
    /// Stable inbox-local id, e.g. `NOM-001`.
    pub id: String,
    pub kind: NominationKind,
    pub title: String,
    pub body: String,
    /// For edge kinds: the existing node the edge points to. For `new-requirement`:
    /// an optional `derives_from` thesis id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Proposed priority for a `new-requirement` (P0/P1/P2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    /// Proposed category for a `new-requirement`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// Provenance, e.g. `pre-commit` or a commit sha — how the nomination arose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub created_at: String,
    pub created_by: String,
}

/// The staging inbox: an ordered list of pending nominations.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Inbox {
    #[serde(default)]
    pub nominations: Vec<Nomination>,
}

impl Inbox {
    /// Allocate the next `NOM-NNN` id, one past the current maximum.
    pub fn next_id(&self) -> String {
        let max = self
            .nominations
            .iter()
            .filter_map(|n| n.id.rsplit('-').next())
            .filter_map(|n| n.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("NOM-{:03}", max + 1)
    }

    pub fn get(&self, id: &str) -> Option<&Nomination> {
        let want = id.to_uppercase();
        self.nominations.iter().find(|n| n.id == want)
    }
}

/// Path to the staging inbox for a given lattice root.
pub fn inbox_path(root: &Path) -> PathBuf {
    root.join(INBOX_FILE)
}

/// Load the inbox, returning an empty one if the file does not exist yet.
pub fn load_inbox(root: &Path) -> Result<Inbox, String> {
    let path = inbox_path(root);
    if !path.exists() {
        return Ok(Inbox::default());
    }
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("failed to read inbox: {}", e))?;
    if content.trim().is_empty() {
        return Ok(Inbox::default());
    }
    serde_yaml::from_str(&content).map_err(|e| format!("failed to parse inbox: {}", e))
}

/// Persist the inbox. When it becomes empty the file is removed to keep the tree clean.
pub fn save_inbox(root: &Path, inbox: &Inbox) -> Result<(), String> {
    let path = inbox_path(root);
    if inbox.nominations.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("failed to remove inbox: {}", e))?;
        }
        return Ok(());
    }
    let content =
        serde_yaml::to_string(inbox).map_err(|e| format!("failed to serialize inbox: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("failed to write inbox: {}", e))
}

/// Append a nomination to the inbox, assigning it a fresh id. Returns the stored copy.
///
/// This writes only to the staging file — it never touches `.lattice/`
/// (the nominate-not-commit guardrail, REQ-CAPTURE-005).
pub fn add_nomination(root: &Path, mut nom: Nomination) -> Result<Nomination, String> {
    let mut inbox = load_inbox(root)?;
    nom.id = inbox.next_id();
    inbox.nominations.push(nom.clone());
    save_inbox(root, &inbox)?;
    Ok(nom)
}

/// Remove a nomination by id, returning it if present.
pub fn remove_nomination(root: &Path, id: &str) -> Result<Option<Nomination>, String> {
    let mut inbox = load_inbox(root)?;
    let want = id.to_uppercase();
    if let Some(pos) = inbox.nominations.iter().position(|n| n.id == want) {
        let removed = inbox.nominations.remove(pos);
        save_inbox(root, &inbox)?;
        Ok(Some(removed))
    } else {
        Ok(None)
    }
}

/// One changed file that intersects a tracked implementation binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingMatch {
    pub file: String,
    pub implementation: String,
    /// Requirements the implementation satisfies (via `satisfies` edges).
    pub requirements: Vec<String>,
}

/// Result of the deterministic relevance prefilter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefilterResult {
    pub changed_files: Vec<String>,
    pub matches: Vec<BindingMatch>,
}

impl PrefilterResult {
    /// True when at least one changed file touches tracked surface — the signal
    /// that escalates to AI nomination.
    pub fn tripped(&self) -> bool {
        !self.matches.is_empty()
    }

    /// Distinct requirement ids implicated across all matches.
    pub fn requirement_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .matches
            .iter()
            .flat_map(|m| m.requirements.iter().cloned())
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }
}

/// Map every bound file path to the implementations that bind it.
pub fn binding_map(nodes: &[&LatticeNode]) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in nodes {
        if node.node_type != NodeType::Implementation {
            continue;
        }
        if let Some(NodeMeta::Implementation(meta)) = &node.meta
            && let Some(files) = &meta.files
        {
            for f in files {
                map.entry(f.path.clone()).or_default().push(node.id.clone());
            }
        }
    }
    map
}

/// Deterministic relevance prefilter (REQ-CAPTURE-002).
///
/// Intersects `changed_files` with implementation binding paths and resolves the
/// affected requirement ids through each implementation's `satisfies` edges. No
/// model call — this is the cheap gate that decides whether AI nomination is
/// even worth invoking.
pub fn prefilter(nodes: &[&LatticeNode], changed_files: &[String]) -> PrefilterResult {
    let bindings = binding_map(nodes);
    let impl_by_id: BTreeMap<&str, &LatticeNode> = nodes
        .iter()
        .filter(|n| n.node_type == NodeType::Implementation)
        .map(|n| (n.id.as_str(), *n))
        .collect();

    let mut matches = Vec::new();
    for file in changed_files {
        let Some(impl_ids) = bindings.get(file) else {
            continue;
        };
        for impl_id in impl_ids {
            let requirements = impl_by_id
                .get(impl_id.as_str())
                .map(|n| {
                    n.edges
                        .as_ref()
                        .and_then(|e| e.satisfies.as_ref())
                        .map(|s| s.iter().map(|r| r.target.clone()).collect())
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            matches.push(BindingMatch {
                file: file.clone(),
                implementation: impl_id.clone(),
                requirements,
            });
        }
    }

    PrefilterResult {
        changed_files: changed_files.to_vec(),
        matches,
    }
}

/// Files staged for commit (`git diff --cached --name-only`).
pub fn staged_files(root: &Path) -> Vec<String> {
    git_name_only(root, &["diff", "--cached", "--name-only"])
}

/// Files changed since a git ref (`git diff --name-only <ref>`).
pub fn changed_files_since(root: &Path, git_ref: &str) -> Vec<String> {
    git_name_only(root, &["diff", "--name-only", git_ref])
}

fn git_name_only(root: &Path, args: &[&str]) -> Vec<String> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        EdgeReference, Edges, FileRef, ImplementationMeta, LatticeNode, NodeType, Status,
    };

    fn impl_node(id: &str, files: &[&str], satisfies: &[&str]) -> LatticeNode {
        LatticeNode {
            id: id.to_string(),
            node_type: NodeType::Implementation,
            title: id.to_string(),
            body: String::new(),
            status: Status::Active,
            version: "1.0.0".to_string(),
            created_at: "now".to_string(),
            created_by: "test".to_string(),
            requested_by: None,
            priority: None,
            category: None,
            tags: None,
            acceptance: None,
            visibility: None,
            resolution: None,
            meta: Some(NodeMeta::Implementation(ImplementationMeta {
                language: None,
                files: Some(
                    files
                        .iter()
                        .map(|p| FileRef {
                            path: p.to_string(),
                            functions: None,
                        })
                        .collect(),
                ),
                test_command: None,
            })),
            edges: Some(Edges {
                satisfies: Some(
                    satisfies
                        .iter()
                        .map(|t| EdgeReference {
                            target: t.to_string(),
                            version: Some("1.0.0".to_string()),
                            rationale: None,
                        })
                        .collect(),
                ),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn prefilter_matches_bound_file_and_resolves_requirements() {
        let a = impl_node("IMP-001", &["src/storage.rs"], &["REQ-CORE-001"]);
        let nodes = vec![&a];
        let changed = vec!["src/storage.rs".to_string(), "README.md".to_string()];
        let result = prefilter(&nodes, &changed);
        assert!(result.tripped());
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].implementation, "IMP-001");
        assert_eq!(result.requirement_ids(), vec!["REQ-CORE-001"]);
    }

    #[test]
    fn prefilter_is_silent_when_no_bound_file_changes() {
        let a = impl_node("IMP-001", &["src/storage.rs"], &["REQ-CORE-001"]);
        let nodes = vec![&a];
        let changed = vec!["README.md".to_string(), "docs/x.md".to_string()];
        let result = prefilter(&nodes, &changed);
        assert!(!result.tripped());
        assert!(result.requirement_ids().is_empty());
    }

    #[test]
    fn inbox_assigns_incrementing_ids() {
        let mut inbox = Inbox::default();
        assert_eq!(inbox.next_id(), "NOM-001");
        inbox.nominations.push(Nomination {
            id: "NOM-001".to_string(),
            kind: NominationKind::NewRequirement,
            title: "t".to_string(),
            body: "b".to_string(),
            target: None,
            priority: None,
            category: None,
            rationale: None,
            source: None,
            created_at: "now".to_string(),
            created_by: "test".to_string(),
        });
        assert_eq!(inbox.next_id(), "NOM-002");
    }

    #[test]
    fn inbox_roundtrip_and_guardrail_stays_outside_lattice() {
        let dir = std::env::temp_dir().join(format!("lattice-capture-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let nom = Nomination {
            id: String::new(),
            kind: NominationKind::Challenges,
            title: "contradiction spotted".to_string(),
            body: "the diff contradicts THX-VERSION-AWARE".to_string(),
            target: Some("THX-VERSION-AWARE".to_string()),
            priority: None,
            category: None,
            rationale: None,
            source: Some("pre-commit".to_string()),
            created_at: "now".to_string(),
            created_by: "agent:test".to_string(),
        };
        let stored = add_nomination(&dir, nom).unwrap();
        assert_eq!(stored.id, "NOM-001");

        // The inbox lives outside .lattice/ (guardrail, REQ-CAPTURE-005).
        assert!(inbox_path(&dir).ends_with("lattice-inbox.yaml"));
        assert!(!inbox_path(&dir).to_string_lossy().contains(".lattice"));

        let loaded = load_inbox(&dir).unwrap();
        assert_eq!(loaded.nominations.len(), 1);
        assert_eq!(
            loaded.get("nom-001").unwrap().kind,
            NominationKind::Challenges
        );

        let removed = remove_nomination(&dir, "NOM-001").unwrap();
        assert!(removed.is_some());
        // File is cleaned up once empty.
        assert!(!inbox_path(&dir).exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn nomination_kind_maps_to_edge_type() {
        assert_eq!(NominationKind::NewRequirement.edge_type(), None);
        assert_eq!(
            NominationKind::RevealsGapIn.edge_type(),
            Some("reveals_gap_in")
        );
        assert_eq!(
            NominationKind::parse("validates").unwrap(),
            NominationKind::Validates
        );
        assert!(NominationKind::parse("bogus").is_err());
    }
}
