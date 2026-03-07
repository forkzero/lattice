//! Search engine for Lattice nodes.
//!
//! Consolidates search logic used by both the CLI (`lattice search`) and MCP
//! (`lattice_search` tool) into a single module. Supports keyword matching,
//! structured filters (priority, resolution, tags, category, id prefix), and
//! graph-proximity filtering via `--related-to`.
//!
//! Linked requirement: REQ-API-008

use crate::graph::build_node_index;
use crate::storage::{load_all_nodes, load_nodes_by_type};
use crate::types::{Edges, LatticeNode, Resolution};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

/// Unified search parameters accepted by both CLI and MCP.
#[derive(Debug, Default, Clone)]
pub struct SearchParams {
    /// Node type to search: sources, theses, requirements, implementations.
    /// Defaults to "requirements".
    pub node_type: Option<String>,
    /// Free-text query matched against title and body (case-insensitive substring).
    pub query: Option<String>,
    /// Filter by priority level (P0, P1, P2).
    pub priority: Option<String>,
    /// Filter by resolution status (verified, blocked, deferred, wontfix, unresolved/open).
    pub resolution: Option<String>,
    /// Filter by a single tag (case-insensitive).
    pub tag: Option<String>,
    /// Filter requiring all specified tags to be present (case-insensitive).
    pub tags: Option<Vec<String>>,
    /// Filter by category (case-insensitive).
    pub category: Option<String>,
    /// Filter by ID prefix (case-insensitive).
    pub id_prefix: Option<String>,
    /// Find nodes related to this node ID via graph proximity.
    pub related_to: Option<String>,
}

/// A single search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub body: String,
    pub version: String,
    pub priority: Option<String>,
    pub resolution: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
}

impl From<&LatticeNode> for SearchResult {
    fn from(n: &LatticeNode) -> Self {
        Self {
            id: n.id.clone(),
            title: n.title.clone(),
            body: n.body.clone(),
            version: n.version.clone(),
            priority: n.priority.as_ref().map(|p| format!("{:?}", p)),
            resolution: n
                .resolution
                .as_ref()
                .map(|r| format!("{:?}", r.status).to_lowercase()),
            category: n.category.clone(),
            tags: n.tags.clone(),
        }
    }
}

/// Aggregated search results.
#[derive(Debug, Clone)]
pub struct SearchResults {
    pub count: usize,
    pub results: Vec<SearchResult>,
}

/// Search engine operating over a lattice root directory.
pub struct SearchEngine {
    root: PathBuf,
}

impl SearchEngine {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Execute a search with the given parameters.
    pub fn search(&self, params: &SearchParams) -> Result<SearchResults, String> {
        let node_type = params.node_type.as_deref().unwrap_or("requirements");
        let type_name = validate_node_type(node_type)?;

        let nodes = load_nodes_by_type(&self.root, type_name).map_err(|e| e.to_string())?;

        let related_ids = self.build_related_ids(params.related_to.as_deref())?;

        let results: Vec<SearchResult> = nodes
            .iter()
            .filter(|n| matches_filters(n, params, related_ids.as_ref()))
            .map(SearchResult::from)
            .collect();

        Ok(SearchResults {
            count: results.len(),
            results,
        })
    }

    /// Build the set of node IDs related to `related_to` via graph proximity.
    fn build_related_ids(
        &self,
        related_to: Option<&str>,
    ) -> Result<Option<HashSet<String>>, String> {
        let related_to_id = match related_to {
            Some(id) => id,
            None => return Ok(None),
        };

        let index = build_node_index(&self.root).map_err(|e| e.to_string())?;
        let source_node = index
            .get(related_to_id)
            .ok_or_else(|| format!("Node not found: {}", related_to_id))?;

        let mut related = HashSet::new();

        // Collect edge targets from the source node
        let source_targets = collect_edge_targets(source_node.edges.as_ref());

        // Find nodes sharing any of these edge targets
        for node in index.values() {
            if node.id == related_to_id {
                continue;
            }
            if let Some(edges) = &node.edges {
                let node_targets = collect_edge_targets(Some(edges));
                if !source_targets.is_disjoint(&node_targets) {
                    related.insert(node.id.clone());
                }
            }
        }

        // Include direct references from the source node
        for target in &source_targets {
            related.insert(target.clone());
        }

        // Include nodes that reference the source node
        for node in index.values() {
            if let Some(edges) = &node.edges
                && edge_targets_id(edges, related_to_id)
            {
                related.insert(node.id.clone());
            }
        }

        Ok(Some(related))
    }
}

// --- Search Index Infrastructure ---

/// Persistent index metadata stored at `~/.cache/lattice/<project-hash>/index.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchIndex {
    /// Embedding model name (empty until Phase 2 adds vector search).
    pub model: String,
    /// Embedding dimension (0 until Phase 2).
    pub dimension: usize,
    /// Map of node_id → SHA-256 of content (title + "\n" + body).
    pub content_hashes: BTreeMap<String, String>,
}

/// Summary of index health returned by `index_status()`.
#[derive(Debug, Clone)]
pub struct IndexStatus {
    /// Whether an index file exists on disk.
    pub exists: bool,
    /// Number of nodes currently indexed.
    pub indexed: usize,
    /// Number of indexed nodes whose content hash no longer matches (stale).
    pub stale: usize,
    /// Number of nodes in the lattice that are not in the index.
    pub missing: usize,
    /// Total nodes in the lattice.
    pub total: usize,
    /// Cache directory path.
    pub cache_dir: PathBuf,
}

impl SearchIndex {
    fn new() -> Self {
        Self {
            model: String::new(),
            dimension: 0,
            content_hashes: BTreeMap::new(),
        }
    }

    fn load(path: &std::path::Path) -> Result<Self, String> {
        let data =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read index: {}", e))?;
        serde_json::from_str(&data).map_err(|e| format!("Failed to parse index: {}", e))
    }

    fn save(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create cache dir: {}", e))?;
        }
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize index: {}", e))?;
        std::fs::write(path, data).map_err(|e| format!("Failed to write index: {}", e))
    }
}

/// Compute SHA-256 of a node's content for change detection.
pub fn content_hash(title: &str, body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    hasher.update(b"\n");
    hasher.update(body.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Compute the cache directory for a lattice project.
/// Returns `~/.cache/lattice/<sha256-of-canonical-root-path>/`.
pub fn cache_dir(root: &std::path::Path) -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let canonical =
        std::fs::canonicalize(root).map_err(|e| format!("Failed to canonicalize root: {}", e))?;
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let project_hash = format!("{:x}", hasher.finalize());
    // Use first 16 hex chars (64 bits) — sufficient for per-machine uniqueness
    Ok(PathBuf::from(home)
        .join(".cache")
        .join("lattice")
        .join(&project_hash[..16]))
}

impl SearchEngine {
    /// Build or rebuild the search index. Hashes all node content and writes
    /// `index.json` to the cache directory. Returns the number of nodes
    /// indexed and the number that were unchanged (cache hits).
    pub fn index_build(&self) -> Result<(usize, usize), String> {
        let nodes = load_all_nodes(&self.root).map_err(|e| e.to_string())?;
        let cache = cache_dir(&self.root)?;
        let index_path = cache.join("index.json");

        // Load existing index for diff comparison
        let old_index = SearchIndex::load(&index_path).ok();

        let mut new_index = SearchIndex::new();
        let mut unchanged = 0;

        for node in &nodes {
            let hash = content_hash(&node.title, &node.body);
            if let Some(ref old) = old_index
                && old.content_hashes.get(&node.id).map(|h| h.as_str()) == Some(&hash)
            {
                unchanged += 1;
            }
            new_index.content_hashes.insert(node.id.clone(), hash);
        }

        new_index.save(&index_path)?;

        Ok((nodes.len(), unchanged))
    }

    /// Report index health: exists, indexed, stale, missing counts.
    pub fn index_status(&self) -> Result<IndexStatus, String> {
        let nodes = load_all_nodes(&self.root).map_err(|e| e.to_string())?;
        let cache = cache_dir(&self.root)?;
        let index_path = cache.join("index.json");

        if !index_path.exists() {
            return Ok(IndexStatus {
                exists: false,
                indexed: 0,
                stale: 0,
                missing: nodes.len(),
                total: nodes.len(),
                cache_dir: cache,
            });
        }

        let index = SearchIndex::load(&index_path)?;
        let mut stale = 0;
        let mut missing = 0;

        for node in &nodes {
            match index.content_hashes.get(&node.id) {
                Some(stored_hash) => {
                    let current = content_hash(&node.title, &node.body);
                    if *stored_hash != current {
                        stale += 1;
                    }
                }
                None => missing += 1,
            }
        }

        Ok(IndexStatus {
            exists: true,
            indexed: index.content_hashes.len(),
            stale,
            missing,
            total: nodes.len(),
            cache_dir: cache,
        })
    }
}

/// Validate and normalize the node type string.
fn validate_node_type(node_type: &str) -> Result<&'static str, String> {
    match node_type {
        "sources" => Ok("sources"),
        "theses" => Ok("theses"),
        "requirements" => Ok("requirements"),
        "implementations" => Ok("implementations"),
        _ => Err(format!(
            "Unknown type: {}. Use: sources, theses, requirements, implementations",
            node_type
        )),
    }
}

/// Check if a node passes all filter criteria.
fn matches_filters(
    node: &LatticeNode,
    params: &SearchParams,
    related_ids: Option<&HashSet<String>>,
) -> bool {
    // ID prefix filter
    if let Some(ref prefix) = params.id_prefix
        && !node.id.to_uppercase().starts_with(&prefix.to_uppercase())
    {
        return false;
    }

    // Graph proximity filter
    if let Some(related) = related_ids
        && !related.contains(&node.id)
    {
        return false;
    }

    // Text search in title and body
    if let Some(ref q) = params.query {
        let q_lower = q.to_lowercase();
        if !node.title.to_lowercase().contains(&q_lower)
            && !node.body.to_lowercase().contains(&q_lower)
        {
            return false;
        }
    }

    // Priority filter
    if let Some(ref p) = params.priority {
        let node_priority = node.priority.as_ref().map(|p| format!("{:?}", p));
        if node_priority.as_deref() != Some(p.to_uppercase().as_str()) {
            return false;
        }
    }

    // Resolution status filter
    if let Some(ref res) = params.resolution {
        let res_lower = res.to_lowercase();
        let matches = match res_lower.as_str() {
            "verified" => matches!(
                node.resolution.as_ref().map(|r| &r.status),
                Some(Resolution::Verified)
            ),
            "blocked" => matches!(
                node.resolution.as_ref().map(|r| &r.status),
                Some(Resolution::Blocked)
            ),
            "deferred" => matches!(
                node.resolution.as_ref().map(|r| &r.status),
                Some(Resolution::Deferred)
            ),
            "wontfix" => matches!(
                node.resolution.as_ref().map(|r| &r.status),
                Some(Resolution::Wontfix)
            ),
            "unresolved" | "open" => node.resolution.is_none(),
            _ => true,
        };
        if !matches {
            return false;
        }
    }

    // Single tag filter
    if let Some(ref t) = params.tag {
        let tag_lower = t.to_lowercase();
        let has_tag = node
            .tags
            .as_ref()
            .map(|tags| tags.iter().any(|t| t.to_lowercase() == tag_lower))
            .unwrap_or(false);
        if !has_tag {
            return false;
        }
    }

    // Tags intersection filter (all specified tags must be present)
    if let Some(ref search_tags) = params.tags {
        let node_tags: HashSet<String> = node
            .tags
            .as_ref()
            .map(|tags| tags.iter().map(|t| t.to_lowercase()).collect())
            .unwrap_or_default();
        for st in search_tags {
            if !node_tags.contains(&st.to_lowercase()) {
                return false;
            }
        }
    }

    // Category filter
    if let Some(ref cat) = params.category {
        let matches_cat = node
            .category
            .as_ref()
            .map(|c| c.to_lowercase() == cat.to_lowercase())
            .unwrap_or(false);
        if !matches_cat {
            return false;
        }
    }

    true
}

/// Collect all edge targets from a node's edges into a set.
fn collect_edge_targets(edges: Option<&Edges>) -> HashSet<String> {
    let mut targets = HashSet::new();
    if let Some(edges) = edges {
        for edge_list in [
            &edges.derives_from,
            &edges.depends_on,
            &edges.satisfies,
            &edges.supported_by,
        ]
        .into_iter()
        .flatten()
        {
            for edge in edge_list {
                targets.insert(edge.target.clone());
            }
        }
    }
    targets
}

/// Check if any edge in the given edges references `target_id`.
fn edge_targets_id(edges: &Edges, target_id: &str) -> bool {
    [
        &edges.derives_from,
        &edges.depends_on,
        &edges.satisfies,
        &edges.supported_by,
    ]
    .iter()
    .any(|edge_list| {
        edge_list
            .as_ref()
            .is_some_and(|v| v.iter().any(|e| e.target == target_id))
    })
}

/// Parse a comma-separated string into a Vec of trimmed strings.
pub fn split_csv(s: Option<String>) -> Option<Vec<String>> {
    s.map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LatticeNode, NodeType, Priority, ResolutionInfo, Status};

    fn make_node(id: &str, title: &str, body: &str) -> LatticeNode {
        LatticeNode {
            id: id.to_string(),
            node_type: NodeType::Requirement,
            title: title.to_string(),
            body: body.to_string(),
            status: Status::Active,
            version: "1.0.0".to_string(),
            created_at: "2024-01-01".to_string(),
            created_by: "test".to_string(),
            requested_by: None,
            priority: None,
            category: None,
            tags: None,
            acceptance: None,
            visibility: None,
            resolution: None,
            meta: None,
            edges: None,
        }
    }

    #[test]
    fn test_matches_query_in_title() {
        let node = make_node("REQ-001", "Version drift detection", "Detect stale edges");
        let params = SearchParams {
            query: Some("drift".to_string()),
            ..Default::default()
        };
        assert!(matches_filters(&node, &params, None));
    }

    #[test]
    fn test_matches_query_in_body() {
        let node = make_node("REQ-001", "Some title", "Detect version drift");
        let params = SearchParams {
            query: Some("drift".to_string()),
            ..Default::default()
        };
        assert!(matches_filters(&node, &params, None));
    }

    #[test]
    fn test_no_match_query() {
        let node = make_node("REQ-001", "Some title", "Some body");
        let params = SearchParams {
            query: Some("drift".to_string()),
            ..Default::default()
        };
        assert!(!matches_filters(&node, &params, None));
    }

    #[test]
    fn test_matches_id_prefix() {
        let node = make_node("REQ-CORE-001", "Title", "Body");
        let params = SearchParams {
            id_prefix: Some("REQ-CORE".to_string()),
            ..Default::default()
        };
        assert!(matches_filters(&node, &params, None));
    }

    #[test]
    fn test_no_match_id_prefix() {
        let node = make_node("REQ-CLI-001", "Title", "Body");
        let params = SearchParams {
            id_prefix: Some("REQ-CORE".to_string()),
            ..Default::default()
        };
        assert!(!matches_filters(&node, &params, None));
    }

    #[test]
    fn test_matches_priority() {
        let mut node = make_node("REQ-001", "Title", "Body");
        node.priority = Some(Priority::P0);
        let params = SearchParams {
            priority: Some("p0".to_string()),
            ..Default::default()
        };
        assert!(matches_filters(&node, &params, None));
    }

    #[test]
    fn test_matches_resolution_verified() {
        let mut node = make_node("REQ-001", "Title", "Body");
        node.resolution = Some(ResolutionInfo {
            status: Resolution::Verified,
            reason: None,
            resolved_by: "test".to_string(),
            resolved_at: "2024-01-01".to_string(),
        });
        let params = SearchParams {
            resolution: Some("verified".to_string()),
            ..Default::default()
        };
        assert!(matches_filters(&node, &params, None));
    }

    #[test]
    fn test_matches_resolution_unresolved() {
        let node = make_node("REQ-001", "Title", "Body");
        let params = SearchParams {
            resolution: Some("unresolved".to_string()),
            ..Default::default()
        };
        assert!(matches_filters(&node, &params, None));
    }

    #[test]
    fn test_matches_single_tag() {
        let mut node = make_node("REQ-001", "Title", "Body");
        node.tags = Some(vec!["core".to_string(), "api".to_string()]);
        let params = SearchParams {
            tag: Some("core".to_string()),
            ..Default::default()
        };
        assert!(matches_filters(&node, &params, None));
    }

    #[test]
    fn test_matches_multiple_tags() {
        let mut node = make_node("REQ-001", "Title", "Body");
        node.tags = Some(vec!["core".to_string(), "api".to_string()]);
        let params = SearchParams {
            tags: Some(vec!["core".to_string(), "api".to_string()]),
            ..Default::default()
        };
        assert!(matches_filters(&node, &params, None));
    }

    #[test]
    fn test_no_match_missing_tag() {
        let mut node = make_node("REQ-001", "Title", "Body");
        node.tags = Some(vec!["core".to_string()]);
        let params = SearchParams {
            tags: Some(vec!["core".to_string(), "api".to_string()]),
            ..Default::default()
        };
        assert!(!matches_filters(&node, &params, None));
    }

    #[test]
    fn test_matches_category() {
        let mut node = make_node("REQ-001", "Title", "Body");
        node.category = Some("core".to_string());
        let params = SearchParams {
            category: Some("Core".to_string()),
            ..Default::default()
        };
        assert!(matches_filters(&node, &params, None));
    }

    #[test]
    fn test_matches_related_ids() {
        let node = make_node("REQ-001", "Title", "Body");
        let mut related = HashSet::new();
        related.insert("REQ-001".to_string());
        let params = SearchParams::default();
        assert!(matches_filters(&node, &params, Some(&related)));
    }

    #[test]
    fn test_no_match_related_ids() {
        let node = make_node("REQ-001", "Title", "Body");
        let mut related = HashSet::new();
        related.insert("REQ-002".to_string());
        let params = SearchParams::default();
        assert!(!matches_filters(&node, &params, Some(&related)));
    }

    #[test]
    fn test_split_csv() {
        assert_eq!(
            split_csv(Some("a, b, c".to_string())),
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
        assert_eq!(split_csv(None), None);
    }

    #[test]
    fn test_validate_node_type() {
        assert!(validate_node_type("requirements").is_ok());
        assert!(validate_node_type("sources").is_ok());
        assert!(validate_node_type("theses").is_ok());
        assert!(validate_node_type("implementations").is_ok());
        assert!(validate_node_type("invalid").is_err());
    }

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("Title", "Body");
        let h2 = content_hash("Title", "Body");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn test_content_hash_differs_on_change() {
        let h1 = content_hash("Title", "Body");
        let h2 = content_hash("Title", "Body changed");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_content_hash_title_body_separator() {
        // "A\nB" should differ from "A\n" + "B" assembled differently
        let h1 = content_hash("A", "B");
        let h2 = content_hash("A\nB", "");
        assert_ne!(
            h1, h2,
            "title+newline+body should differ from title containing newline"
        );
    }

    #[test]
    fn test_search_index_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.json");

        let mut index = SearchIndex::new();
        index
            .content_hashes
            .insert("REQ-001".to_string(), "abc123".to_string());
        index
            .content_hashes
            .insert("REQ-002".to_string(), "def456".to_string());

        index.save(&path).unwrap();
        let loaded = SearchIndex::load(&path).unwrap();

        assert_eq!(loaded.content_hashes.len(), 2);
        assert_eq!(loaded.content_hashes.get("REQ-001").unwrap(), "abc123");
        assert_eq!(loaded.model, "");
        assert_eq!(loaded.dimension, 0);
    }

    #[test]
    fn test_search_index_load_missing_file() {
        let result = SearchIndex::load(std::path::Path::new("/nonexistent/index.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_cache_dir_deterministic() {
        // Use a real path that exists for canonicalize
        let dir = tempfile::tempdir().unwrap();
        let d1 = cache_dir(dir.path()).unwrap();
        let d2 = cache_dir(dir.path()).unwrap();
        assert_eq!(d1, d2);
    }

    #[test]
    fn test_cache_dir_differs_by_project() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();
        let d1 = cache_dir(dir1.path()).unwrap();
        let d2 = cache_dir(dir2.path()).unwrap();
        assert_ne!(d1, d2);
    }
}
