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
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use crate::storage::{AddEdgeOptions, AddRequirementOptions, add_edge, add_requirement};
use crate::types::{LatticeNode, NodeIndex, NodeMeta, NodeType, Priority, Status};

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

    /// Index of the nomination with this id (case-insensitive) — the single
    /// source of truth for id matching, shared by `get` and `remove_nomination`.
    pub fn position(&self, id: &str) -> Option<usize> {
        let want = id.to_uppercase();
        self.nominations.iter().position(|n| n.id == want)
    }

    pub fn get(&self, id: &str) -> Option<&Nomination> {
        self.position(id).map(|i| &self.nominations[i])
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
    if let Some(pos) = inbox.position(id) {
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

/// An implementation's binding: its id and the requirements it satisfies,
/// resolved once so a match can clone the cached list rather than re-walk edges.
struct Binding {
    implementation: String,
    requirements: Vec<String>,
}

/// Deterministic relevance prefilter (REQ-CAPTURE-002).
///
/// Intersects `changed_files` with implementation binding paths and resolves the
/// affected requirement ids through each implementation's `satisfies` edges. No
/// model call — this is the cheap gate that decides whether AI nomination is
/// even worth invoking.
pub fn prefilter(nodes: &[&LatticeNode], changed_files: &[String]) -> PrefilterResult {
    // Single pass: bound file path -> implementations binding it (id + satisfied
    // requirements resolved up front). No second index, no id round-trip.
    let mut bindings: BTreeMap<&str, Vec<Binding>> = BTreeMap::new();
    for node in nodes {
        if node.node_type != NodeType::Implementation {
            continue;
        }
        let Some(NodeMeta::Implementation(meta)) = &node.meta else {
            continue;
        };
        let Some(files) = &meta.files else { continue };
        let requirements: Vec<String> = node
            .edges
            .as_ref()
            .and_then(|e| e.satisfies.as_ref())
            .map(|s| s.iter().map(|r| r.target.clone()).collect())
            .unwrap_or_default();
        for f in files {
            bindings.entry(f.path.as_str()).or_default().push(Binding {
                implementation: node.id.clone(),
                requirements: requirements.clone(),
            });
        }
    }

    let mut matches = Vec::new();
    for file in changed_files {
        let Some(binds) = bindings.get(file.as_str()) else {
            continue;
        };
        for bind in binds {
            matches.push(BindingMatch {
                file: file.clone(),
                implementation: bind.implementation.clone(),
                requirements: bind.requirements.clone(),
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

/// The agent-facing nomination bundle for a `scan` (REQ-CAPTURE-003).
///
/// Pairs the deterministic prefilter result with the affected requirements
/// (enriched with title/body) and the standing instruction for how an agent
/// should nominate. The prompt policy lives here with the pipeline it governs,
/// not in the CLI print path.
pub fn scan_bundle(index: &NodeIndex, result: &PrefilterResult) -> Value {
    let requirements: Vec<Value> = result
        .requirement_ids()
        .iter()
        .map(|id| match index.get(id) {
            Some(n) => json!({"id": id, "title": n.title, "body": n.body}),
            None => json!({ "id": id }),
        })
        .collect();
    json!({
        "tripped": result.tripped(),
        "changed_files": result.changed_files,
        "matches": result.matches,
        "requirements": requirements,
        "instructions": "For each affected requirement, judge whether this change is lattice-worthy: does it satisfy, contradict (challenges), reveal a gap in (reveals-gap-in), or validate a tracked node, or warrant a new requirement? Emit at most 3 high-confidence nominations via 'lattice capture add'. Returning none is valid.",
    })
}

/// Materialize a nomination into the graph (REQ-CAPTURE-001) — the only path
/// that mutates `.lattice/`, invoked exclusively on explicit human `accept`
/// (the nominate-not-commit guardrail, REQ-CAPTURE-005). Returns a summary of
/// what was created. Does not touch the inbox; the caller clears the nomination.
pub fn accept(
    root: &Path,
    nom: &Nomination,
    id_assign: Option<&str>,
    from: Option<&str>,
) -> Result<String, String> {
    match nom.kind {
        NominationKind::NewRequirement => {
            let req_id =
                id_assign.ok_or("new-requirement needs --id-assign REQ-XXX to materialize")?;
            let priority = nom
                .priority
                .as_deref()
                .map(Priority::from_str)
                .transpose()?
                .unwrap_or(Priority::P1);
            let opts = AddRequirementOptions {
                id: req_id.to_string(),
                title: nom.title.clone(),
                body: nom.body.clone(),
                priority,
                category: nom
                    .category
                    .clone()
                    .unwrap_or_else(|| "CAPTURE".to_string()),
                tags: None,
                derives_from: nom.target.clone().map(|t| vec![t]),
                depends_on: None,
                status: Status::Active,
                created_by: nom.created_by.clone(),
            };
            add_requirement(root, opts).map_err(|e| e.to_string())?;
            Ok(format!("Created requirement {}", req_id))
        }
        _ => {
            let edge_type = nom.kind.edge_type().expect("edge kind has an edge type");
            let from_id = from.ok_or_else(|| {
                format!("{} needs --from <ID> (the source node)", nom.kind.as_str())
            })?;
            let to_id = nom
                .target
                .as_deref()
                .ok_or("edge nomination has no target node")?;
            add_edge(
                root,
                AddEdgeOptions {
                    from_id: from_id.to_string(),
                    edge_type: edge_type.to_string(),
                    to_id: to_id.to_string(),
                    rationale: nom.rationale.clone().or_else(|| Some(nom.body.clone())),
                },
            )
            .map_err(|e| e.to_string())?;
            Ok(format!(
                "Added edge {} --[{}]--> {}",
                from_id, edge_type, to_id
            ))
        }
    }
}

/// Outcome of installing one git hook.
pub enum HookOutcome {
    Installed(PathBuf),
    SkippedExisting(PathBuf),
    Failed(PathBuf, String),
}

/// Install non-blocking pre-commit and pre-push hooks that drive capture
/// (REQ-CAPTURE-003, REQ-CAPTURE-004). The script bodies — and the policy they
/// encode (pre-commit never blocks; pre-push hard-gates on `health --check`) —
/// live here with the pipeline, not in the CLI. Existing unmanaged hooks are
/// left alone unless `force` is set.
pub fn install_hooks(root: &Path, force: bool) -> Result<Vec<HookOutcome>, String> {
    const MARKER: &str = "# lattice-capture";
    let hooks_dir = Command::new("git")
        .args(["rev-parse", "--git-path", "hooks"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| root.join(s))
        .unwrap_or_else(|| root.join(".git").join("hooks"));
    std::fs::create_dir_all(&hooks_dir)
        .map_err(|e| format!("could not create hooks dir: {}", e))?;

    let pre_commit = format!(
        "#!/bin/sh\n\
         {MARKER} pre-commit nominator (REQ-CAPTURE-003)\n\
         # Non-blocking: surfaces tracked-surface changes; never fails the commit.\n\
         lattice capture scan --staged || true\n\
         exit 0\n"
    );
    let pre_push = format!(
        "#!/bin/sh\n\
         {MARKER} pre-push gate (REQ-CAPTURE-004)\n\
         if ! lattice health --check; then\n\
         \techo 'lattice: health check failed — push blocked (run: lattice health)' >&2\n\
         \texit 1\n\
         fi\n\
         lattice capture gate || true\n\
         exit 0\n"
    );

    let mut outcomes = Vec::new();
    for (name, contents) in [("pre-commit", pre_commit), ("pre-push", pre_push)] {
        let path = hooks_dir.join(name);
        if path.exists() {
            let existing = std::fs::read_to_string(&path).unwrap_or_default();
            if !existing.contains(MARKER) && !force {
                outcomes.push(HookOutcome::SkippedExisting(path));
                continue;
            }
        }
        match write_hook(&path, &contents) {
            Ok(()) => outcomes.push(HookOutcome::Installed(path)),
            Err(e) => outcomes.push(HookOutcome::Failed(path, e)),
        }
    }
    Ok(outcomes)
}

fn write_hook(path: &Path, contents: &str) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
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

    fn nomination(kind: NominationKind, target: Option<&str>) -> Nomination {
        Nomination {
            id: "NOM-001".to_string(),
            kind,
            title: "t".to_string(),
            body: "b".to_string(),
            target: target.map(|s| s.to_string()),
            priority: None,
            category: None,
            rationale: None,
            source: None,
            created_at: "now".to_string(),
            created_by: "test".to_string(),
        }
    }

    #[test]
    fn accept_requires_id_and_from_before_touching_the_graph() {
        let root = std::env::temp_dir(); // never written — guards fire first
        let new_req = nomination(NominationKind::NewRequirement, None);
        let err = accept(&root, &new_req, None, None).unwrap_err();
        assert!(err.contains("id-assign"), "got: {err}");

        let edge = nomination(NominationKind::Challenges, Some("THX-X"));
        let err = accept(&root, &edge, None, None).unwrap_err();
        assert!(err.contains("--from"), "got: {err}");
    }

    #[test]
    fn scan_bundle_reports_matches_and_enriches_requirements() {
        let a = impl_node("IMP-001", &["src/storage.rs"], &["REQ-CORE-001"]);
        let mut req = impl_node("REQ-CORE-001", &[], &[]);
        req.node_type = NodeType::Requirement;
        req.title = "Storage layer".to_string();
        let mut index = NodeIndex::new();
        index.insert(a.id.clone(), a.clone());
        index.insert(req.id.clone(), req.clone());
        let nodes: Vec<&LatticeNode> = index.values().collect();

        let result = prefilter(&nodes, &["src/storage.rs".to_string()]);
        let bundle = scan_bundle(&index, &result);
        assert_eq!(bundle["tripped"], serde_json::json!(true));
        assert_eq!(bundle["requirements"][0]["id"], "REQ-CORE-001");
        assert_eq!(bundle["requirements"][0]["title"], "Storage layer");
    }
}
