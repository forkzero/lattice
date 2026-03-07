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
use std::io::{Read as _, Write as _};
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
    /// Similarity score from semantic search. `None` for keyword search results.
    pub score: Option<f32>,
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
            score: None,
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

        let mut results: Vec<SearchResult> = nodes
            .iter()
            .filter(|n| matches_filters(n, params, related_ids.as_ref()))
            .map(|n| {
                let mut r = SearchResult::from(n);
                if let Some(ref q) = params.query {
                    let score = keyword_score(n, q);
                    r.score = Some(score);
                }
                r
            })
            .collect();

        // Sort by keyword score descending when a query is present
        if params.query.is_some() {
            results.sort_by(|a, b| {
                b.score
                    .unwrap_or(0.0)
                    .partial_cmp(&a.score.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

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

/// Score a keyword match: title match = 2.0, body match = 1.0, additive.
fn keyword_score(node: &LatticeNode, query: &str) -> f32 {
    let q = query.to_lowercase();
    let mut score = 0.0;
    if node.title.to_lowercase().contains(&q) {
        score += 2.0;
    }
    if node.body.to_lowercase().contains(&q) {
        score += 1.0;
    }
    score
}

/// Parse a comma-separated string into a Vec of trimmed strings.
pub fn split_csv(s: Option<String>) -> Option<Vec<String>> {
    s.map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
}

/// Reciprocal Rank Fusion: merge two ranked lists into a single fused ranking.
/// `score(node) = Σ 1/(k + rank)` across both lists (1-indexed ranks).
#[cfg_attr(not(feature = "vector-search"), allow(dead_code))]
fn reciprocal_rank_fusion(
    keyword_ranked: &[(String, f32)],
    semantic_ranked: &[(String, f32)],
    k: usize,
) -> Vec<(String, f32)> {
    use std::collections::HashMap;
    let mut scores: HashMap<String, f32> = HashMap::new();

    for (rank, (id, _)) in keyword_ranked.iter().enumerate() {
        *scores.entry(id.clone()).or_default() += 1.0 / (k + rank + 1) as f32;
    }
    for (rank, (id, _)) in semantic_ranked.iter().enumerate() {
        *scores.entry(id.clone()).or_default() += 1.0 / (k + rank + 1) as f32;
    }

    let mut fused: Vec<(String, f32)> = scores.into_iter().collect();
    fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    fused
}

// --- Embedding Infrastructure ---

/// Trait for embedding providers. Allows swapping models in the future.
pub trait EmbeddingProvider {
    /// Model name for index metadata.
    fn model_name(&self) -> &str;
    /// Embedding dimension.
    fn dimension(&self) -> usize;
    /// Generate embeddings for a batch of texts.
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;
}

/// fastembed-based embedding provider (feature-gated).
#[cfg(feature = "vector-search")]
pub struct FastEmbedProvider {
    model: fastembed::TextEmbedding,
    model_name: String,
    dimension: usize,
}

#[cfg(feature = "vector-search")]
impl FastEmbedProvider {
    pub fn new() -> Result<Self, String> {
        use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

        let mut init =
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true);

        // Support air-gapped environments via LATTICE_EMBED_CACHE_DIR
        if let Ok(cache_dir) = std::env::var("LATTICE_EMBED_CACHE_DIR") {
            init = init.with_cache_dir(std::path::PathBuf::from(cache_dir));
        }

        let model = TextEmbedding::try_new(init)
            .map_err(|e| format!("Failed to load embedding model: {}", e))?;

        Ok(Self {
            model,
            model_name: "all-MiniLM-L6-v2".to_string(),
            dimension: 384,
        })
    }
}

#[cfg(feature = "vector-search")]
impl EmbeddingProvider for FastEmbedProvider {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        self.model
            .embed(texts.to_vec(), None)
            .map_err(|e| format!("Embedding failed: {}", e))
    }
}

/// Read embeddings from binary file.
/// Format: repeated `[node_id_len: u16, node_id: [u8], embedding: [f32; dim]]`
pub fn load_embeddings(
    path: &std::path::Path,
    dimension: usize,
) -> Result<BTreeMap<String, Vec<f32>>, String> {
    let data = std::fs::read(path).map_err(|e| format!("Failed to read embeddings: {}", e))?;
    let mut cursor = std::io::Cursor::new(data);
    let mut result = BTreeMap::new();

    loop {
        let mut len_buf = [0u8; 2];
        if cursor.read_exact(&mut len_buf).is_err() {
            break; // EOF
        }
        let id_len = u16::from_le_bytes(len_buf) as usize;

        let mut id_buf = vec![0u8; id_len];
        cursor
            .read_exact(&mut id_buf)
            .map_err(|e| format!("Truncated embedding file: {}", e))?;
        let node_id = String::from_utf8(id_buf)
            .map_err(|e| format!("Invalid node ID in embedding: {}", e))?;

        let mut emb = Vec::with_capacity(dimension);
        for _ in 0..dimension {
            let mut buf = [0u8; 4];
            cursor
                .read_exact(&mut buf)
                .map_err(|e| format!("Truncated embedding data: {}", e))?;
            emb.push(f32::from_le_bytes(buf));
        }

        result.insert(node_id, emb);
    }

    Ok(result)
}

/// Write embeddings to binary file.
pub fn save_embeddings(
    path: &std::path::Path,
    embeddings: &BTreeMap<String, Vec<f32>>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create cache dir: {}", e))?;
    }
    let mut file =
        std::fs::File::create(path).map_err(|e| format!("Failed to create embeddings: {}", e))?;

    for (node_id, embedding) in embeddings {
        let id_bytes = node_id.as_bytes();
        file.write_all(&(id_bytes.len() as u16).to_le_bytes())
            .map_err(|e| format!("Write error: {}", e))?;
        file.write_all(id_bytes)
            .map_err(|e| format!("Write error: {}", e))?;
        for &val in embedding {
            file.write_all(&val.to_le_bytes())
                .map_err(|e| format!("Write error: {}", e))?;
        }
    }

    Ok(())
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

impl SearchEngine {
    /// Build or rebuild the search index with embeddings.
    /// When an EmbeddingProvider is given, generates embeddings for changed nodes.
    pub fn index_build_with_embeddings(
        &self,
        provider: &dyn EmbeddingProvider,
    ) -> Result<(usize, usize), String> {
        let nodes = load_all_nodes(&self.root).map_err(|e| e.to_string())?;
        let cache = cache_dir(&self.root)?;
        let index_path = cache.join("index.json");
        let embeddings_path = cache.join("embeddings.bin");

        // Load existing index and embeddings for diff comparison
        let old_index = SearchIndex::load(&index_path).ok();
        let mut old_embeddings = if embeddings_path.exists() {
            load_embeddings(&embeddings_path, provider.dimension()).ok()
        } else {
            None
        };

        let mut new_index = SearchIndex::new();
        new_index.model = provider.model_name().to_string();
        new_index.dimension = provider.dimension();

        let mut unchanged = 0;
        let mut to_embed: Vec<(String, String)> = Vec::new(); // (node_id, text)
        let mut new_embeddings: BTreeMap<String, Vec<f32>> = BTreeMap::new();

        for node in &nodes {
            let hash = content_hash(&node.title, &node.body);
            let text = format!("{}\n{}", node.title, node.body);

            let is_unchanged = old_index
                .as_ref()
                .and_then(|idx| idx.content_hashes.get(&node.id))
                .map(|h| h.as_str())
                == Some(&hash);

            if is_unchanged {
                // Move existing embedding if available (avoids clone)
                if let Some(ref mut old_embs) = old_embeddings
                    && let Some(emb) = old_embs.remove(&node.id)
                {
                    new_embeddings.insert(node.id.clone(), emb);
                    unchanged += 1;
                    new_index.content_hashes.insert(node.id.clone(), hash);
                    continue;
                }
            }

            // Need to (re-)embed this node
            to_embed.push((node.id.clone(), text));
            new_index.content_hashes.insert(node.id.clone(), hash);
        }

        // Batch embed changed nodes
        if !to_embed.is_empty() {
            let texts: Vec<String> = to_embed.iter().map(|(_, t)| t.clone()).collect();
            let embeddings = provider.embed(&texts)?;
            for ((node_id, _), embedding) in to_embed.iter().zip(embeddings.into_iter()) {
                new_embeddings.insert(node_id.clone(), embedding);
            }
        }

        new_index.save(&index_path)?;
        save_embeddings(&embeddings_path, &new_embeddings)?;

        Ok((nodes.len(), unchanged))
    }

    /// Perform semantic search using pre-built embeddings.
    /// Embeds the query, computes cosine similarity against all indexed nodes,
    /// and returns the top_k results sorted by score.
    pub fn semantic_search(
        &self,
        query: &str,
        top_k: usize,
        provider: &dyn EmbeddingProvider,
    ) -> Result<Vec<SearchResult>, String> {
        let cache = cache_dir(&self.root)?;
        let index_path = cache.join("index.json");
        let embeddings_path = cache.join("embeddings.bin");

        let index = SearchIndex::load(&index_path).map_err(|_| {
            "No search index found. Run 'lattice search --index' to build it.".to_string()
        })?;

        if index.dimension == 0 {
            return Err(
                "Index has no embeddings. Rebuild with 'lattice search --index'.".to_string(),
            );
        }

        // Validate model/dimension match the current provider
        if index.dimension != provider.dimension() {
            return Err(format!(
                "Index dimension ({}) does not match provider dimension ({}). Rebuild with 'lattice search --index'.",
                index.dimension,
                provider.dimension()
            ));
        }
        if index.model != provider.model_name() {
            return Err(format!(
                "Index model '{}' does not match provider model '{}'. Rebuild with 'lattice search --index'.",
                index.model,
                provider.model_name()
            ));
        }

        let embeddings = load_embeddings(&embeddings_path, index.dimension)?;

        // Embed the query
        let query_embeddings = provider.embed(&[query.to_string()])?;
        let query_vec = query_embeddings
            .into_iter()
            .next()
            .ok_or("Failed to embed query")?;

        // Load all nodes for result metadata
        let all_nodes = load_all_nodes(&self.root).map_err(|e| e.to_string())?;
        let node_map: BTreeMap<&str, &LatticeNode> =
            all_nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        // Score all indexed nodes
        let mut scored: Vec<(String, f32)> = embeddings
            .iter()
            .map(|(id, emb)| (id.clone(), cosine_similarity(&query_vec, emb)))
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        let results = scored
            .into_iter()
            .filter_map(|(id, score)| {
                let node = node_map.get(id.as_str())?;
                let mut result = SearchResult::from(*node);
                result.score = Some(score);
                Some(result)
            })
            .collect();

        Ok(results)
    }

    /// Hybrid search: fuses keyword and semantic rankings via RRF.
    /// Structured filters are applied before both rankings.
    #[cfg(feature = "vector-search")]
    pub fn hybrid_search(
        &self,
        params: &SearchParams,
        provider: &dyn EmbeddingProvider,
    ) -> Result<SearchResults, String> {
        let query = params
            .query
            .as_deref()
            .ok_or("hybrid search requires a query")?;

        let node_type = params.node_type.as_deref().unwrap_or("requirements");
        let type_name = validate_node_type(node_type)?;
        let nodes = load_nodes_by_type(&self.root, type_name).map_err(|e| e.to_string())?;
        let related_ids = self.build_related_ids(params.related_to.as_deref())?;

        // Apply structured filters first
        let filtered: Vec<&LatticeNode> = nodes
            .iter()
            .filter(|n| matches_filters(n, params, related_ids.as_ref()))
            .collect();

        // Keyword ranking on filtered set
        let mut keyword_ranked: Vec<(String, f32)> = filtered
            .iter()
            .map(|n| (n.id.clone(), keyword_score(n, query)))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        keyword_ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Semantic ranking on filtered set
        let cache = cache_dir(&self.root)?;
        let index_path = cache.join("index.json");
        let embeddings_path = cache.join("embeddings.bin");

        let index = SearchIndex::load(&index_path).map_err(|_| {
            "No search index found. Run 'lattice search --index' to build it.".to_string()
        })?;

        if index.dimension == 0 {
            return Err(
                "Index has no embeddings. Rebuild with 'lattice search --index'.".to_string(),
            );
        }

        let embeddings = load_embeddings(&embeddings_path, index.dimension)?;
        let query_embeddings = provider.embed(&[query.to_string()])?;
        let query_vec = query_embeddings
            .into_iter()
            .next()
            .ok_or("Failed to embed query")?;

        let filtered_ids: HashSet<&str> = filtered.iter().map(|n| n.id.as_str()).collect();
        let mut semantic_ranked: Vec<(String, f32)> = embeddings
            .iter()
            .filter(|(id, _)| filtered_ids.contains(id.as_str()))
            .map(|(id, emb)| (id.clone(), cosine_similarity(&query_vec, emb)))
            .collect();
        semantic_ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Fuse via RRF
        let fused = reciprocal_rank_fusion(&keyword_ranked, &semantic_ranked, 60);

        // Build node map for metadata
        let node_map: BTreeMap<&str, &LatticeNode> =
            filtered.iter().map(|n| (n.id.as_str(), *n)).collect();

        let results: Vec<SearchResult> = fused
            .into_iter()
            .filter_map(|(id, score)| {
                let node = node_map.get(id.as_str())?;
                let mut result = SearchResult::from(*node);
                result.score = Some(score);
                Some(result)
            })
            .collect();

        Ok(SearchResults {
            count: results.len(),
            results,
        })
    }
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

    // --- Phase 2: Embedding & Semantic Search Tests ---

    /// Mock embedding provider that returns deterministic embeddings
    /// based on simple text hashing (no ML model required).
    struct MockEmbedProvider;

    impl EmbeddingProvider for MockEmbedProvider {
        fn model_name(&self) -> &str {
            "mock-model"
        }

        fn dimension(&self) -> usize {
            4
        }

        fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Ok(texts
                .iter()
                .map(|t| {
                    // Simple deterministic "embedding" from text bytes
                    let bytes: Vec<u8> = t.bytes().collect();
                    let sum: f32 = bytes.iter().map(|&b| b as f32).sum();
                    let len = bytes.len().max(1) as f32;
                    vec![
                        sum / len,                                  // mean byte value
                        (bytes.len() as f32),                       // length
                        bytes.first().copied().unwrap_or(0) as f32, // first byte
                        bytes.last().copied().unwrap_or(0) as f32,  // last byte
                    ]
                })
                .collect())
        }
    }

    /// Provider that always fails, for error-path testing.
    struct FailingEmbedProvider;

    impl EmbeddingProvider for FailingEmbedProvider {
        fn model_name(&self) -> &str {
            "failing-model"
        }

        fn dimension(&self) -> usize {
            4
        }

        fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Err("embedding failed on purpose".to_string())
        }
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &a);
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "Identical vectors should have similarity 1.0"
        );
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            sim.abs() < 1e-6,
            "Orthogonal vectors should have similarity 0.0"
        );
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim + 1.0).abs() < 1e-6,
            "Opposite vectors should have similarity -1.0"
        );
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![1.0, 2.0, 3.0];
        let zero = vec![0.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &zero);
        assert_eq!(sim, 0.0, "Zero vector should produce similarity 0.0");
    }

    #[test]
    fn test_cosine_similarity_scaled() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![2.0, 4.0, 6.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "Scaled vectors should have similarity 1.0"
        );
    }

    #[test]
    fn test_embeddings_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("embeddings.bin");

        let mut embeddings = BTreeMap::new();
        embeddings.insert("NODE-001".to_string(), vec![1.0f32, 2.0, 3.0, 4.0]);
        embeddings.insert("NODE-002".to_string(), vec![5.0f32, 6.0, 7.0, 8.0]);

        save_embeddings(&path, &embeddings).unwrap();
        let loaded = load_embeddings(&path, 4).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded["NODE-001"], vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(loaded["NODE-002"], vec![5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn test_embeddings_roundtrip_single() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("emb.bin");

        let mut embeddings = BTreeMap::new();
        embeddings.insert("X".to_string(), vec![0.5f32, -0.5]);

        save_embeddings(&path, &embeddings).unwrap();
        let loaded = load_embeddings(&path, 2).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded["X"], vec![0.5, -0.5]);
    }

    #[test]
    fn test_embeddings_roundtrip_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.bin");

        let embeddings = BTreeMap::new();
        save_embeddings(&path, &embeddings).unwrap();
        let loaded = load_embeddings(&path, 4).unwrap();

        assert!(loaded.is_empty());
    }

    #[test]
    fn test_load_embeddings_missing_file() {
        let result = load_embeddings(std::path::Path::new("/nonexistent/emb.bin"), 4);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_embeddings_truncated_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.bin");
        // Write a valid id length header but no id bytes
        std::fs::write(&path, [5u8, 0]).unwrap();
        let result = load_embeddings(&path, 4);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Truncated"));
    }

    #[test]
    fn test_mock_embed_provider() {
        let provider = MockEmbedProvider;
        assert_eq!(provider.model_name(), "mock-model");
        assert_eq!(provider.dimension(), 4);

        let results = provider
            .embed(&["hello".to_string(), "world".to_string()])
            .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].len(), 4);
        assert_eq!(results[1].len(), 4);
        // Different inputs produce different embeddings
        assert_ne!(results[0], results[1]);
    }

    /// Helper: create a minimal lattice directory with a few nodes.
    fn setup_lattice_with_nodes(dir: &std::path::Path) {
        use crate::storage::init_lattice;

        init_lattice(dir, false).unwrap();

        let req1 = LatticeNode {
            id: "REQ-TEST-001".to_string(),
            node_type: NodeType::Requirement,
            title: "Drift detection for version changes".to_string(),
            body: "The system must detect when node versions drift from edge bindings.".to_string(),
            status: Status::Active,
            version: "1.0.0".to_string(),
            created_at: "2024-01-01".to_string(),
            created_by: "test".to_string(),
            priority: Some(Priority::P0),
            category: Some("CORE".to_string()),
            tags: Some(vec!["drift".to_string()]),
            requested_by: None,
            acceptance: None,
            visibility: None,
            resolution: None,
            meta: None,
            edges: None,
        };
        let req2 = LatticeNode {
            id: "REQ-TEST-002".to_string(),
            node_type: NodeType::Requirement,
            title: "Export narrative for stakeholders".to_string(),
            body: "Support exporting the lattice as a readable narrative document.".to_string(),
            status: Status::Active,
            version: "1.0.0".to_string(),
            created_at: "2024-01-01".to_string(),
            created_by: "test".to_string(),
            priority: Some(Priority::P1),
            category: Some("EXPORT".to_string()),
            tags: Some(vec!["export".to_string()]),
            requested_by: None,
            acceptance: None,
            visibility: None,
            resolution: None,
            meta: None,
            edges: None,
        };
        let req3 = LatticeNode {
            id: "REQ-TEST-003".to_string(),
            node_type: NodeType::Requirement,
            title: "Search and filter nodes".to_string(),
            body: "Provide search capabilities with keyword and tag filtering.".to_string(),
            status: Status::Active,
            version: "1.0.0".to_string(),
            created_at: "2024-01-01".to_string(),
            created_by: "test".to_string(),
            priority: Some(Priority::P1),
            category: Some("API".to_string()),
            tags: Some(vec!["search".to_string()]),
            requested_by: None,
            acceptance: None,
            visibility: None,
            resolution: None,
            meta: None,
            edges: None,
        };

        for node in [&req1, &req2, &req3] {
            let type_dir = dir.join(".lattice/requirements");
            let file_path = type_dir.join(format!("{}.yaml", node.id));
            let yaml = serde_yaml::to_string(node).unwrap();
            std::fs::write(file_path, yaml).unwrap();
        }
    }

    #[test]
    fn test_index_build_with_embeddings() {
        let dir = tempfile::tempdir().unwrap();
        setup_lattice_with_nodes(dir.path());

        let engine = SearchEngine::new(dir.path());
        let provider = MockEmbedProvider;

        let (total, unchanged) = engine.index_build_with_embeddings(&provider).unwrap();
        assert_eq!(total, 3);
        assert_eq!(unchanged, 0, "First build should have 0 unchanged");

        // Verify index and embeddings were written
        let cache = cache_dir(&engine.root).unwrap();
        let index = SearchIndex::load(&cache.join("index.json")).unwrap();
        assert_eq!(index.model, "mock-model");
        assert_eq!(index.dimension, 4);
        assert_eq!(index.content_hashes.len(), 3);

        let embeddings = load_embeddings(&cache.join("embeddings.bin"), 4).unwrap();
        assert_eq!(embeddings.len(), 3);
    }

    #[test]
    fn test_index_build_with_embeddings_caches_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        setup_lattice_with_nodes(dir.path());

        let engine = SearchEngine::new(dir.path());
        let provider = MockEmbedProvider;

        // First build
        let (total, unchanged) = engine.index_build_with_embeddings(&provider).unwrap();
        assert_eq!(total, 3);
        assert_eq!(unchanged, 0);

        // Second build with no changes — all should be unchanged
        let (total, unchanged) = engine.index_build_with_embeddings(&provider).unwrap();
        assert_eq!(total, 3);
        assert_eq!(
            unchanged, 3,
            "Rebuild with no changes should reuse all embeddings"
        );
    }

    #[test]
    fn test_index_build_with_embeddings_detects_changes() {
        let dir = tempfile::tempdir().unwrap();
        setup_lattice_with_nodes(dir.path());

        let engine = SearchEngine::new(dir.path());
        let provider = MockEmbedProvider;

        // First build
        engine.index_build_with_embeddings(&provider).unwrap();

        // Modify one node
        let mut node: LatticeNode = {
            let yaml =
                std::fs::read_to_string(dir.path().join(".lattice/requirements/REQ-TEST-001.yaml"))
                    .unwrap();
            serde_yaml::from_str(&yaml).unwrap()
        };
        node.body = "Updated body text for drift detection requirement.".to_string();
        let yaml = serde_yaml::to_string(&node).unwrap();
        std::fs::write(
            dir.path().join(".lattice/requirements/REQ-TEST-001.yaml"),
            yaml,
        )
        .unwrap();

        // Rebuild — 1 changed, 2 unchanged
        let (total, unchanged) = engine.index_build_with_embeddings(&provider).unwrap();
        assert_eq!(total, 3);
        assert_eq!(unchanged, 2, "Only modified node should be re-embedded");
    }

    #[test]
    fn test_index_build_with_embeddings_failing_provider() {
        let dir = tempfile::tempdir().unwrap();
        setup_lattice_with_nodes(dir.path());

        let engine = SearchEngine::new(dir.path());
        let provider = FailingEmbedProvider;

        let result = engine.index_build_with_embeddings(&provider);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("embedding failed on purpose"));
    }

    #[test]
    fn test_semantic_search_returns_ranked_results() {
        let dir = tempfile::tempdir().unwrap();
        setup_lattice_with_nodes(dir.path());

        let engine = SearchEngine::new(dir.path());
        let provider = MockEmbedProvider;

        // Build index first
        engine.index_build_with_embeddings(&provider).unwrap();

        // Search — should return results sorted by score descending
        let results = engine
            .semantic_search("drift detection", 10, &provider)
            .unwrap();
        assert!(!results.is_empty(), "Should return at least one result");
        assert!(results.len() <= 3, "Should not exceed total nodes");

        // Verify scores are in descending order
        for w in results.windows(2) {
            assert!(
                w[0].score.unwrap() >= w[1].score.unwrap(),
                "Results should be sorted by score descending: {:?} >= {:?}",
                w[0].score,
                w[1].score
            );
        }

        // Verify result fields are populated
        let first = &results[0];
        assert!(!first.id.is_empty());
        assert!(!first.title.is_empty());
        assert!(!first.version.is_empty());
    }

    #[test]
    fn test_semantic_search_respects_top_k() {
        let dir = tempfile::tempdir().unwrap();
        setup_lattice_with_nodes(dir.path());

        let engine = SearchEngine::new(dir.path());
        let provider = MockEmbedProvider;

        engine.index_build_with_embeddings(&provider).unwrap();

        let results = engine.semantic_search("test query", 1, &provider).unwrap();
        assert_eq!(results.len(), 1, "top_k=1 should return exactly 1 result");
    }

    #[test]
    fn test_semantic_search_without_index_fails() {
        let dir = tempfile::tempdir().unwrap();
        setup_lattice_with_nodes(dir.path());

        let engine = SearchEngine::new(dir.path());
        let provider = MockEmbedProvider;

        // Don't build index — search should fail gracefully
        let result = engine.semantic_search("test", 10, &provider);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No search index found"));
    }

    #[test]
    fn test_semantic_search_result_has_metadata() {
        let dir = tempfile::tempdir().unwrap();
        setup_lattice_with_nodes(dir.path());

        let engine = SearchEngine::new(dir.path());
        let provider = MockEmbedProvider;

        engine.index_build_with_embeddings(&provider).unwrap();

        let results = engine.semantic_search("drift", 10, &provider).unwrap();

        // Find the drift detection requirement
        let drift_result = results.iter().find(|r| r.id == "REQ-TEST-001");
        assert!(drift_result.is_some(), "Should find REQ-TEST-001");

        let dr = drift_result.unwrap();
        assert_eq!(dr.title, "Drift detection for version changes");
        assert_eq!(dr.category.as_deref(), Some("CORE"));
        assert_eq!(dr.priority.as_deref(), Some("P0"));
        assert!(dr.score.unwrap() > 0.0, "Score should be positive");
    }

    // --- Phase 3: Score Fusion Tests ---

    #[test]
    fn test_keyword_score_title_only() {
        let node = make_node("REQ-001", "Version drift detection", "Some other text");
        assert_eq!(keyword_score(&node, "drift"), 2.0);
    }

    #[test]
    fn test_keyword_score_body_only() {
        let node = make_node("REQ-001", "Some title", "Detect version drift");
        assert_eq!(keyword_score(&node, "drift"), 1.0);
    }

    #[test]
    fn test_keyword_score_both() {
        let node = make_node("REQ-001", "Drift detection", "Detect drift in edges");
        assert_eq!(keyword_score(&node, "drift"), 3.0);
    }

    #[test]
    fn test_keyword_score_neither() {
        let node = make_node("REQ-001", "Some title", "Some body");
        assert_eq!(keyword_score(&node, "drift"), 0.0);
    }

    #[test]
    fn test_keyword_score_case_insensitive() {
        let node = make_node("REQ-001", "DRIFT Detection", "body");
        assert_eq!(keyword_score(&node, "drift"), 2.0);
        assert_eq!(keyword_score(&node, "DRIFT"), 2.0);
    }

    #[test]
    fn test_keyword_search_ranked() {
        let dir = tempfile::tempdir().unwrap();
        setup_lattice_with_nodes(dir.path());

        let engine = SearchEngine::new(dir.path());
        let params = SearchParams {
            node_type: Some("requirements".to_string()),
            query: Some("search".to_string()),
            ..Default::default()
        };

        let results = engine.search(&params).unwrap();
        // REQ-TEST-003 has "Search" in title + "search" in body = 3.0
        // Other nodes should not match
        assert!(!results.results.is_empty());
        assert!(results.results[0].score.unwrap() > 0.0);

        // Verify descending score order
        for w in results.results.windows(2) {
            assert!(w[0].score.unwrap() >= w[1].score.unwrap());
        }
    }

    #[test]
    fn test_reciprocal_rank_fusion_basic() {
        let keyword = vec![
            ("A".to_string(), 3.0),
            ("B".to_string(), 2.0),
            ("C".to_string(), 1.0),
        ];
        let semantic = vec![
            ("B".to_string(), 0.9),
            ("C".to_string(), 0.8),
            ("A".to_string(), 0.7),
        ];

        let fused = reciprocal_rank_fusion(&keyword, &semantic, 60);
        assert_eq!(fused.len(), 3);
        // All three should be present
        let ids: Vec<&str> = fused.iter().map(|(id, _)| id.as_str()).collect();
        assert!(ids.contains(&"A"));
        assert!(ids.contains(&"B"));
        assert!(ids.contains(&"C"));

        // B is rank 2 in keyword (1/(60+2)) and rank 1 in semantic (1/(60+1))
        // A is rank 1 in keyword (1/(60+1)) and rank 3 in semantic (1/(60+3))
        // B should score higher than A since it has better average rank
        let b_score = fused.iter().find(|(id, _)| id == "B").unwrap().1;
        let a_score = fused.iter().find(|(id, _)| id == "A").unwrap().1;
        assert!(
            b_score > a_score,
            "B (rank 2+1) should score higher than A (rank 1+3)"
        );
    }

    #[test]
    fn test_reciprocal_rank_fusion_disjoint() {
        let keyword = vec![("A".to_string(), 3.0)];
        let semantic = vec![("B".to_string(), 0.9)];

        let fused = reciprocal_rank_fusion(&keyword, &semantic, 60);
        assert_eq!(fused.len(), 2);
        // Both should appear with equal RRF scores (each appears once at rank 1)
        assert!((fused[0].1 - fused[1].1).abs() < 1e-6);
    }

    #[test]
    fn test_reciprocal_rank_fusion_single_list() {
        let keyword = vec![("A".to_string(), 3.0), ("B".to_string(), 2.0)];
        let semantic: Vec<(String, f32)> = vec![];

        let fused = reciprocal_rank_fusion(&keyword, &semantic, 60);
        assert_eq!(fused.len(), 2);
        // A at rank 1 should score higher than B at rank 2
        assert!(fused[0].1 > fused[1].1);
        assert_eq!(fused[0].0, "A");
    }

    #[test]
    fn test_reciprocal_rank_fusion_empty() {
        let keyword: Vec<(String, f32)> = vec![];
        let semantic: Vec<(String, f32)> = vec![];

        let fused = reciprocal_rank_fusion(&keyword, &semantic, 60);
        assert!(fused.is_empty());
    }

    #[test]
    #[cfg(feature = "vector-search")]
    fn test_hybrid_search() {
        let dir = tempfile::tempdir().unwrap();
        setup_lattice_with_nodes(dir.path());

        let engine = SearchEngine::new(dir.path());
        let provider = MockEmbedProvider;

        // Build index first
        engine.index_build_with_embeddings(&provider).unwrap();

        let params = SearchParams {
            node_type: Some("requirements".to_string()),
            query: Some("drift".to_string()),
            ..Default::default()
        };

        let results = engine.hybrid_search(&params, &provider).unwrap();
        assert!(!results.results.is_empty());

        // All results should have fused scores
        for r in &results.results {
            assert!(r.score.is_some());
            assert!(r.score.unwrap() > 0.0);
        }

        // Results should be sorted by score descending
        for w in results.results.windows(2) {
            assert!(w[0].score.unwrap() >= w[1].score.unwrap());
        }
    }

    #[test]
    fn test_min_score_filtering() {
        let dir = tempfile::tempdir().unwrap();
        setup_lattice_with_nodes(dir.path());

        let engine = SearchEngine::new(dir.path());
        let params = SearchParams {
            node_type: Some("requirements".to_string()),
            query: Some("drift".to_string()),
            ..Default::default()
        };

        let results = engine.search(&params).unwrap();
        // Should have some results with scores
        assert!(!results.results.is_empty());

        // Filter with a high threshold — should exclude low-scoring results
        let high_threshold = 2.5;
        let filtered: Vec<_> = results
            .results
            .iter()
            .filter(|r| r.score.unwrap_or(0.0) >= high_threshold)
            .collect();

        // REQ-TEST-001 has "drift" in title (2.0) + body (1.0) = 3.0, should pass
        // Others without "drift" shouldn't be in the result set at all
        assert!(
            filtered.len() <= results.results.len(),
            "Filtering should not increase result count"
        );
        for r in &filtered {
            assert!(r.score.unwrap() >= high_threshold);
        }
    }
}
