//! File-based storage for Lattice nodes.
//!
//! Linked requirements: REQ-CORE-004, REQ-CLI-002, REQ-AGENT-002

use crate::types::{
    EdgeReference, Edges, LatticeNode, NodeMeta, NodeType, Priority, Reliability, Resolution,
    ResolutionInfo, SourceMeta, Status, ThesisCategory, ThesisMeta,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use walkdir::WalkDir;

pub const LATTICE_DIR: &str = ".lattice";

/// Git remote info (owner/repo name and description).
pub struct GitRepoInfo {
    pub name: String,
    pub description: String,
}

/// Get the git remote origin URL.
fn get_git_remote_url() -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    let url = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if url.is_empty() { None } else { Some(url) }
}

/// Parse owner/repo from a git remote URL.
/// Handles both HTTPS and SSH formats:
/// - "https://github.com/forkzero/lattice.git" → ("forkzero", "lattice")
/// - "git@github.com:forkzero/lattice.git" → ("forkzero", "lattice")
fn parse_owner_repo(url: &str) -> Option<(String, String)> {
    let cleaned = url.trim_end_matches('/').trim_end_matches(".git");
    // Try HTTPS format: https://github.com/owner/repo
    if let Some(path) = cleaned.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = path.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    // Try SSH format: git@github.com:owner/repo
    if let Some(path) = cleaned.strip_prefix("git@github.com:") {
        let parts: Vec<&str> = path.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    None
}

/// Get repo name from git remote URL.
fn get_git_repo_name() -> Option<String> {
    let url = get_git_remote_url()?;
    parse_owner_repo(&url).map(|(_, repo)| repo).or_else(|| {
        // Fallback: last path segment
        url.trim_end_matches('/')
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
    })
}

/// Derive the GitHub Pages base URL from git remote.
/// e.g. "https://github.com/forkzero/lattice" → "https://forkzero.github.io/lattice"
pub fn get_github_pages_url() -> Option<String> {
    let url = get_git_remote_url()?;
    let (owner, repo) = parse_owner_repo(&url)?;
    Some(format!("https://{}.github.io/{}", owner, repo))
}

/// Try to get repo description from GitHub API via `gh`.
fn get_gh_repo_description() -> Option<String> {
    let output = Command::new("gh")
        .args([
            "repo",
            "view",
            "--json",
            "description",
            "--jq",
            ".description",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let desc = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if desc.is_empty() { None } else { Some(desc) }
}

/// Get repo info from git remote and GitHub API.
pub fn get_git_repo_info() -> GitRepoInfo {
    let name = get_git_repo_name()
        .or_else(|| {
            // Fallback: use current directory name
            std::env::current_dir()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        })
        .unwrap_or_else(|| "my-project".to_string());
    let description = get_gh_repo_description().unwrap_or_default();
    GitRepoInfo { name, description }
}

/// Lattice project configuration read from config.yaml.
#[derive(Debug, serde::Deserialize, serde::Serialize, Default)]
pub struct LatticeConfig {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub description: String,
}

/// Read config.yaml from a lattice root.
pub fn load_config(root: &Path) -> LatticeConfig {
    let config_path = root.join(LATTICE_DIR).join("config.yaml");
    fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| serde_yaml::from_str(&content).ok())
        .unwrap_or_default()
}

fn build_config_yaml(info: &GitRepoInfo) -> String {
    let mut config = String::from("# Lattice configuration\nversion: \"1.0\"\n");
    config.push_str(&format!("project: \"{}\"\n", info.name));
    if !info.description.is_empty() {
        config.push_str(&format!("description: \"{}\"\n", info.description));
    }
    config
}

/// Get the git user name and email from git config.
pub fn get_git_user() -> Option<String> {
    let name = Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let email = Command::new("git")
        .args(["config", "user.email"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    match (name, email) {
        (Some(n), Some(e)) => Some(format!("{} <{}>", n, e)),
        (Some(n), None) => Some(n),
        (None, Some(e)) => Some(e),
        (None, None) => None,
    }
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Not in a lattice directory")]
    NotInLattice,
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    #[error("Invalid node type: {0}")]
    InvalidNodeType(String),
    #[error("{0}")]
    AlreadyExists(String),
}

/// Find the lattice root by searching upward for .lattice directory.
pub fn find_lattice_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let lattice_dir = current.join(LATTICE_DIR);
        if lattice_dir.is_dir() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Initialize a new lattice in the given directory.
pub fn init_lattice(root: &Path, force: bool) -> Result<Vec<PathBuf>, StorageError> {
    let lattice_dir = root.join(LATTICE_DIR);

    // Check if already exists
    if lattice_dir.exists() && !force {
        return Err(StorageError::AlreadyExists(
            "Lattice already initialized. Use --force to overwrite.".to_string(),
        ));
    }

    // Remove existing if force
    if lattice_dir.exists() && force {
        fs::remove_dir_all(&lattice_dir)?;
    }

    let mut created = Vec::new();

    // Create directories
    let dirs = ["sources", "theses", "requirements", "implementations"];
    for dir in &dirs {
        let path = lattice_dir.join(dir);
        fs::create_dir_all(&path)?;
        created.push(path);
    }

    // Create config.yaml with repo info
    let config_path = lattice_dir.join("config.yaml");
    let repo_info = get_git_repo_info();
    let config_content = build_config_yaml(&repo_info);
    fs::write(&config_path, config_content)?;
    created.push(config_path);

    Ok(created)
}

/// Load a single node from a YAML file.
pub fn load_node(path: &Path) -> Result<LatticeNode, StorageError> {
    let content = fs::read_to_string(path)?;
    let node: LatticeNode = serde_yaml::from_str(&content)?;
    Ok(node)
}

/// Save a node to a YAML file.
pub fn save_node(path: &Path, node: &LatticeNode) -> Result<(), StorageError> {
    let content = serde_yaml::to_string(node)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

/// Load all nodes of a specific type from the lattice.
pub fn load_nodes_by_type(root: &Path, node_type: &str) -> Result<Vec<LatticeNode>, StorageError> {
    let type_dir = root.join(LATTICE_DIR).join(node_type);
    if !type_dir.exists() {
        return Ok(Vec::new());
    }

    let mut nodes = Vec::new();
    for entry in WalkDir::new(&type_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "yaml" || ext == "yml")
                .unwrap_or(false)
        })
    {
        match load_node(entry.path()) {
            Ok(node) => nodes.push(node),
            Err(e) => eprintln!("Warning: failed to load {}: {}", entry.path().display(), e),
        }
    }

    Ok(nodes)
}

/// Load all nodes from the lattice.
pub fn load_all_nodes(root: &Path) -> Result<Vec<LatticeNode>, StorageError> {
    let mut all_nodes = Vec::new();
    for type_name in &["sources", "theses", "requirements", "implementations"] {
        let nodes = load_nodes_by_type(root, type_name)?;
        all_nodes.extend(nodes);
    }
    Ok(all_nodes)
}

/// Options for adding a requirement.
pub struct AddRequirementOptions {
    pub id: String,
    pub title: String,
    pub body: String,
    pub priority: Priority,
    pub category: String,
    pub tags: Option<Vec<String>>,
    pub derives_from: Option<Vec<String>>,
    pub depends_on: Option<Vec<String>>,
    pub status: Status,
    pub created_by: String,
}

/// Options for adding a thesis.
pub struct AddThesisOptions {
    pub id: String,
    pub title: String,
    pub body: String,
    pub category: ThesisCategory,
    pub confidence: Option<f64>,
    pub supported_by: Option<Vec<String>>,
    pub status: Status,
    pub created_by: String,
}

/// Options for adding an implementation.
pub struct AddImplementationOptions {
    pub id: String,
    pub title: String,
    pub body: String,
    pub language: Option<String>,
    pub files: Option<Vec<String>>,
    pub test_command: Option<String>,
    pub satisfies: Option<Vec<String>>,
    pub status: Status,
    pub created_by: String,
}

/// Options for adding a source.
pub struct AddSourceOptions {
    pub id: String,
    pub title: String,
    pub body: String,
    pub url: Option<String>,
    pub citations: Option<Vec<String>>,
    pub reliability: Reliability,
    pub status: Status,
    pub created_by: String,
}

/// Check that a node ID doesn't already exist in the lattice. Returns an error if it does.
fn check_duplicate_id(root: &Path, id: &str) -> Result<(), StorageError> {
    if find_node_path(root, id).is_ok() {
        return Err(StorageError::AlreadyExists(format!(
            "Node with ID '{}' already exists",
            id
        )));
    }
    Ok(())
}

fn make_edge_refs(targets: Option<Vec<String>>) -> Option<Vec<EdgeReference>> {
    targets.map(|t| {
        t.into_iter()
            .map(|target| EdgeReference {
                target,
                version: Some("1.0.0".to_string()),
                rationale: None,
            })
            .collect()
    })
}

fn slugify(s: &str, max_len: usize) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(max_len)
        .collect()
}

/// Add a requirement to the lattice.
pub fn add_requirement(
    root: &Path,
    options: AddRequirementOptions,
) -> Result<PathBuf, StorageError> {
    check_duplicate_id(root, &options.id)?;
    let now = chrono::Utc::now().to_rfc3339();

    let edges = Edges {
        derives_from: make_edge_refs(options.derives_from),
        depends_on: make_edge_refs(options.depends_on),
        ..Default::default()
    };

    let node = LatticeNode {
        id: options.id.clone(),
        node_type: NodeType::Requirement,
        title: options.title.clone(),
        body: options.body,
        status: options.status,
        version: "1.0.0".to_string(),
        created_at: now,
        created_by: options.created_by,
        requested_by: get_git_user(),
        priority: Some(options.priority),
        category: Some(options.category.clone()),
        tags: options.tags,
        acceptance: None,
        visibility: None,
        resolution: None,
        meta: None,
        edges: Some(edges),
    };

    let category_dir = options.category.to_lowercase();
    let id_number = options.id.split('-').next_back().unwrap_or("000");
    let slug = slugify(&options.title, 40);
    let file_name = format!("{}-{}.yaml", id_number, slug);
    let file_path = root
        .join(LATTICE_DIR)
        .join("requirements")
        .join(&category_dir)
        .join(&file_name);

    save_node(&file_path, &node)?;
    Ok(file_path)
}

/// Add a thesis to the lattice.
pub fn add_thesis(root: &Path, options: AddThesisOptions) -> Result<PathBuf, StorageError> {
    check_duplicate_id(root, &options.id)?;
    let now = chrono::Utc::now().to_rfc3339();

    let edges = Edges {
        supported_by: make_edge_refs(options.supported_by),
        ..Default::default()
    };

    let node = LatticeNode {
        id: options.id.clone(),
        node_type: NodeType::Thesis,
        title: options.title,
        body: options.body,
        status: options.status,
        version: "1.0.0".to_string(),
        created_at: now,
        created_by: options.created_by,
        requested_by: get_git_user(),
        priority: None,
        category: None,
        tags: None,
        acceptance: None,
        visibility: None,
        resolution: None,
        meta: Some(NodeMeta::Thesis(ThesisMeta {
            category: options.category,
            confidence: options.confidence,
        })),
        edges: Some(edges),
    };

    let slug = options
        .id
        .to_lowercase()
        .trim_start_matches("thx-")
        .to_string();
    let file_name = format!("{}.yaml", slug);
    let file_path = root.join(LATTICE_DIR).join("theses").join(&file_name);

    save_node(&file_path, &node)?;
    Ok(file_path)
}

/// Add a source to the lattice.
pub fn add_source(root: &Path, options: AddSourceOptions) -> Result<PathBuf, StorageError> {
    check_duplicate_id(root, &options.id)?;
    let now = chrono::Utc::now().to_rfc3339();
    let today = now.split('T').next().unwrap_or(&now).to_string();

    let node = LatticeNode {
        id: options.id.clone(),
        node_type: NodeType::Source,
        title: options.title,
        body: options.body,
        status: options.status,
        version: "1.0.0".to_string(),
        created_at: now,
        created_by: options.created_by,
        requested_by: get_git_user(),
        priority: None,
        category: None,
        tags: None,
        acceptance: None,
        visibility: None,
        resolution: None,
        meta: Some(NodeMeta::Source(SourceMeta {
            url: options.url,
            citations: options.citations,
            reliability: Some(options.reliability),
            retrieved_at: Some(today),
        })),
        edges: None,
    };

    let slug = options
        .id
        .to_lowercase()
        .trim_start_matches("src-")
        .to_string();
    let file_name = format!("{}.yaml", slug);
    let file_path = root.join(LATTICE_DIR).join("sources").join(&file_name);

    save_node(&file_path, &node)?;
    Ok(file_path)
}

/// Add an implementation to the lattice.
pub fn add_implementation(
    root: &Path,
    options: AddImplementationOptions,
) -> Result<PathBuf, StorageError> {
    check_duplicate_id(root, &options.id)?;
    let now = chrono::Utc::now().to_rfc3339();

    let edges = Edges {
        satisfies: make_edge_refs(options.satisfies),
        ..Default::default()
    };

    let files = options.files.map(|paths| {
        paths
            .into_iter()
            .map(|p| crate::types::FileRef {
                path: p,
                functions: None,
            })
            .collect()
    });

    let node = LatticeNode {
        id: options.id.clone(),
        node_type: NodeType::Implementation,
        title: options.title,
        body: options.body,
        status: options.status,
        version: "1.0.0".to_string(),
        created_at: now,
        created_by: options.created_by,
        requested_by: get_git_user(),
        priority: None,
        category: None,
        tags: None,
        acceptance: None,
        visibility: None,
        resolution: None,
        meta: Some(NodeMeta::Implementation(crate::types::ImplementationMeta {
            language: options.language,
            files,
            test_command: options.test_command,
        })),
        edges: Some(edges),
    };

    let slug = options
        .id
        .to_lowercase()
        .trim_start_matches("imp-")
        .to_string();
    let file_name = format!("{}.yaml", slug);
    let file_path = root
        .join(LATTICE_DIR)
        .join("implementations")
        .join(&file_name);

    save_node(&file_path, &node)?;
    Ok(file_path)
}

/// Find the file path for a node by ID.
pub fn find_node_path(root: &Path, node_id: &str) -> Result<PathBuf, StorageError> {
    let lattice_dir = root.join(LATTICE_DIR);

    for type_name in &["sources", "theses", "requirements", "implementations"] {
        let type_dir = lattice_dir.join(type_name);
        if !type_dir.exists() {
            continue;
        }

        for entry in WalkDir::new(&type_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "yaml" || ext == "yml")
                    .unwrap_or(false)
            })
        {
            if load_node(entry.path()).is_ok_and(|node| node.id == node_id) {
                return Ok(entry.path().to_path_buf());
            }
        }
    }

    Err(StorageError::NodeNotFound(node_id.to_string()))
}

/// Options for resolving a node.
pub struct ResolveOptions {
    pub node_id: String,
    pub resolution: Resolution,
    pub reason: Option<String>,
    pub resolved_by: String,
}

/// Resolve a node with a status.
pub fn resolve_node(root: &Path, options: ResolveOptions) -> Result<PathBuf, StorageError> {
    let path = find_node_path(root, &options.node_id)?;
    let mut node = load_node(&path)?;

    let now = chrono::Utc::now().to_rfc3339();

    node.resolution = Some(ResolutionInfo {
        status: options.resolution,
        reason: options.reason,
        resolved_at: now,
        resolved_by: options.resolved_by,
    });

    save_node(&path, &node)?;
    Ok(path)
}

/// Gap type for requirement refinement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GapType {
    Clarification,
    DesignDecision,
    MissingRequirement,
    Contradiction,
}

impl std::fmt::Display for GapType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GapType::Clarification => write!(f, "clarification"),
            GapType::DesignDecision => write!(f, "design_decision"),
            GapType::MissingRequirement => write!(f, "missing_requirement"),
            GapType::Contradiction => write!(f, "contradiction"),
        }
    }
}

impl std::str::FromStr for GapType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "clarification" => Ok(GapType::Clarification),
            "design_decision" => Ok(GapType::DesignDecision),
            "missing_requirement" => Ok(GapType::MissingRequirement),
            "contradiction" => Ok(GapType::Contradiction),
            _ => Err(format!(
                "Invalid gap type: {}. Expected: clarification, design_decision, missing_requirement, contradiction",
                s
            )),
        }
    }
}

/// Options for refining a requirement (creating a sub-requirement from a gap).
pub struct RefineOptions {
    pub parent_id: String,
    pub gap_type: GapType,
    pub title: String,
    pub description: String,
    pub proposed: Option<String>,
    pub implementation_id: Option<String>,
    pub created_by: String,
}

/// Result of a refinement operation.
pub struct RefineResult {
    pub sub_requirement_path: PathBuf,
    pub sub_requirement_id: String,
    pub parent_updated: bool,
    pub implementation_updated: bool,
}

/// Refine a requirement by creating a sub-requirement that captures a gap.
///
/// This creates a sub-requirement with ID `{PARENT}-A`, `{PARENT}-B`, etc.,
/// wires `depends_on` from parent to sub-req, and optionally adds a
/// `reveals_gap_in` edge from the implementation to the parent.
pub fn refine_requirement(
    root: &Path,
    options: RefineOptions,
) -> Result<RefineResult, StorageError> {
    // Load the parent requirement
    let parent_path = find_node_path(root, &options.parent_id)?;
    let mut parent_node = load_node(&parent_path)?;

    // Determine the next suffix letter by scanning existing sub-requirements
    let suffix = next_suffix(root, &options.parent_id)?;
    let sub_id = format!("{}-{}", options.parent_id, suffix);

    // Build body with gap metadata
    let mut body = options.description.clone();
    if let Some(ref proposed) = options.proposed {
        body.push_str(&format!("\n\n## Proposed Resolution\n\n{}", proposed));
    }
    body.push_str(&format!("\n\n---\n_Gap type: {}_", options.gap_type));

    // Determine status: clarifications with a proposal auto-resolve, others are draft
    let status = if options.gap_type == GapType::Clarification && options.proposed.is_some() {
        Status::Active
    } else {
        Status::Draft
    };

    // Inherit derives_from from parent
    let parent_derives = parent_node
        .edges
        .as_ref()
        .and_then(|e| e.derives_from.clone());

    // Inherit category and priority from parent
    let category = parent_node
        .category
        .clone()
        .unwrap_or_else(|| "AGENT".to_string());
    let priority = parent_node.priority.clone().unwrap_or(Priority::P1);

    let edges = Edges {
        derives_from: parent_derives,
        ..Default::default()
    };

    let now = chrono::Utc::now().to_rfc3339();
    let sub_node = LatticeNode {
        id: sub_id.clone(),
        node_type: NodeType::Requirement,
        title: options.title.clone(),
        body,
        status,
        version: "1.0.0".to_string(),
        created_at: now,
        created_by: options.created_by,
        requested_by: get_git_user(),
        priority: Some(priority),
        category: Some(category.clone()),
        tags: Some(vec!["refinement".to_string(), options.gap_type.to_string()]),
        acceptance: None,
        visibility: None,
        resolution: None,
        meta: None,
        edges: Some(edges),
    };

    // Save the sub-requirement
    let category_dir = category.to_lowercase();
    let slug = slugify(&options.title, 40);
    let file_name = format!("{}-{}.yaml", suffix.to_lowercase(), slug);
    let sub_path = root
        .join(LATTICE_DIR)
        .join("requirements")
        .join(&category_dir)
        .join(&file_name);
    save_node(&sub_path, &sub_node)?;

    // Add depends_on edge from parent to sub-requirement
    let parent_edges = parent_node.edges.get_or_insert_with(Edges::default);
    let dep_edge = EdgeReference {
        target: sub_id.clone(),
        version: Some("1.0.0".to_string()),
        rationale: Some(format!("Refinement: {}", options.gap_type)),
    };
    if let Some(deps) = &mut parent_edges.depends_on {
        deps.push(dep_edge);
    } else {
        parent_edges.depends_on = Some(vec![dep_edge]);
    }
    save_node(&parent_path, &parent_node)?;

    // If implementation provided, add reveals_gap_in edge
    let mut impl_updated = false;
    if let Some(ref impl_id) = options.implementation_id
        && let Ok(impl_path) = find_node_path(root, impl_id)
    {
        let mut impl_node = load_node(&impl_path)?;
        let impl_edges = impl_node.edges.get_or_insert_with(Edges::default);
        let gap_edge = EdgeReference {
            target: options.parent_id.clone(),
            version: Some(parent_node.version.clone()),
            rationale: Some(format!("{}: {}", options.gap_type, options.title)),
        };
        if let Some(gaps) = &mut impl_edges.reveals_gap_in {
            gaps.push(gap_edge);
        } else {
            impl_edges.reveals_gap_in = Some(vec![gap_edge]);
        }
        save_node(&impl_path, &impl_node)?;
        impl_updated = true;
    }

    Ok(RefineResult {
        sub_requirement_path: sub_path,
        sub_requirement_id: sub_id,
        parent_updated: true,
        implementation_updated: impl_updated,
    })
}

/// Determine the next suffix letter (A, B, C, ...) for sub-requirements.
fn next_suffix(root: &Path, parent_id: &str) -> Result<String, StorageError> {
    let all_nodes = load_all_nodes(root)?;
    let prefix = format!("{}-", parent_id);
    let mut max_suffix: u8 = b'A' - 1; // Start before 'A'

    for node in &all_nodes {
        if let Some(rest) = node.id.strip_prefix(&prefix) {
            // Only match single-letter suffixes (A, B, C, ...)
            if rest.len() == 1
                && let Some(ch) = rest.chars().next()
                && ch.is_ascii_uppercase()
                && ch as u8 > max_suffix
            {
                max_suffix = ch as u8;
            }
        }
    }

    if max_suffix < b'A' {
        Ok("A".to_string())
    } else if max_suffix < b'Z' {
        Ok(((max_suffix + 1) as char).to_string())
    } else {
        Err(StorageError::AlreadyExists(
            "Too many sub-requirements (max 26)".to_string(),
        ))
    }
}

/// Options for verifying an implementation satisfies a requirement.
pub struct VerifyOptions {
    pub implementation_id: String,
    pub requirement_id: String,
    pub tests_pass: bool,
    pub coverage: Option<f64>,
    pub files: Option<Vec<String>>,
    pub verified_by: String,
}

/// Record that an implementation satisfies a requirement.
///
/// Creates or updates a `satisfies` edge from the implementation to the
/// requirement, bound to the requirement's current version. Records
/// evidence (test results, coverage) as rationale on the edge.
pub fn verify_implementation(root: &Path, options: VerifyOptions) -> Result<PathBuf, StorageError> {
    // Load the requirement to get its current version
    let req_path = find_node_path(root, &options.requirement_id)?;
    let req_node = load_node(&req_path)?;

    // Load the implementation
    let impl_path = find_node_path(root, &options.implementation_id)?;
    let mut impl_node = load_node(&impl_path)?;

    // Build rationale from evidence
    let mut evidence = Vec::new();
    if options.tests_pass {
        evidence.push("tests pass".to_string());
    }
    if let Some(cov) = options.coverage {
        evidence.push(format!("coverage: {:.0}%", cov * 100.0));
    }
    if let Some(ref files) = options.files {
        evidence.push(format!("files: {}", files.join(", ")));
    }
    let rationale = if evidence.is_empty() {
        None
    } else {
        Some(evidence.join("; "))
    };

    let new_edge = EdgeReference {
        target: options.requirement_id.clone(),
        version: Some(req_node.version.clone()),
        rationale,
    };

    // Update or create the satisfies edge
    let edges = impl_node.edges.get_or_insert_with(Edges::default);

    if let Some(satisfies) = &mut edges.satisfies {
        // Update existing edge or add new one
        if let Some(existing) = satisfies
            .iter_mut()
            .find(|e| e.target == options.requirement_id)
        {
            existing.version = Some(req_node.version.clone());
            existing.rationale = new_edge.rationale;
        } else {
            satisfies.push(new_edge);
        }
    } else {
        edges.satisfies = Some(vec![new_edge]);
    }

    save_node(&impl_path, &impl_node)?;
    Ok(impl_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_init_lattice_creates_structure() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let created = init_lattice(root, false).unwrap();

        assert!(root.join(LATTICE_DIR).exists());
        assert!(root.join(LATTICE_DIR).join("config.yaml").exists());
        assert!(root.join(LATTICE_DIR).join("sources").exists());
        assert!(root.join(LATTICE_DIR).join("theses").exists());
        assert!(root.join(LATTICE_DIR).join("requirements").exists());
        assert!(root.join(LATTICE_DIR).join("implementations").exists());
        assert_eq!(created.len(), 5);
    }

    #[test]
    fn test_init_lattice_fails_if_exists() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // First init should succeed
        init_lattice(root, false).unwrap();

        // Second init should fail
        let result = init_lattice(root, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_init_lattice_force_overwrites() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // First init
        init_lattice(root, false).unwrap();

        // Create a file in the lattice to verify it gets removed
        let marker = root.join(LATTICE_DIR).join("marker.txt");
        fs::write(&marker, "test").unwrap();
        assert!(marker.exists());

        // Force init should succeed and remove marker
        init_lattice(root, true).unwrap();
        assert!(!marker.exists());
    }

    #[test]
    fn test_find_lattice_root_finds_parent() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        init_lattice(root, false).unwrap();

        // Create a nested directory
        let nested = root.join("src").join("deep");
        fs::create_dir_all(&nested).unwrap();

        // Should find root from nested
        let found = find_lattice_root(&nested);
        assert!(found.is_some());
        assert_eq!(found.unwrap(), root);
    }

    #[test]
    fn test_find_lattice_root_returns_none() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // No lattice initialized
        let found = find_lattice_root(root);
        assert!(found.is_none());
    }

    #[test]
    fn test_get_git_user_returns_some_or_none() {
        // This test just verifies the function doesn't panic
        // It may return Some or None depending on git config
        let _result = get_git_user();
    }

    #[test]
    fn test_load_node_from_valid_yaml() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.yaml");
        fs::write(
            &file,
            "id: REQ-TEST\ntype: requirement\ntitle: Test\nbody: Body\nstatus: active\nversion: '1.0.0'\ncreated_at: '2026-01-01'\ncreated_by: test\n",
        ).unwrap();

        let node = load_node(&file).unwrap();
        assert_eq!(node.id, "REQ-TEST");
        assert_eq!(node.title, "Test");
    }

    #[test]
    fn test_load_node_from_invalid_yaml() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("bad.yaml");
        fs::write(&file, "not: valid: yaml: {{").unwrap();

        assert!(load_node(&file).is_err());
    }

    #[test]
    fn test_load_node_missing_file() {
        let result = load_node(Path::new("/nonexistent/file.yaml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_save_and_load_node_roundtrip() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("node.yaml");

        let node = crate::types::LatticeNode {
            id: "REQ-RT".to_string(),
            node_type: crate::types::NodeType::Requirement,
            title: "Roundtrip Test".to_string(),
            body: "Test body".to_string(),
            status: crate::types::Status::Active,
            version: "1.0.0".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            created_by: "test".to_string(),
            requested_by: None,
            priority: Some(crate::types::Priority::P0),
            category: Some("CORE".to_string()),
            tags: Some(vec!["test".to_string()]),
            acceptance: None,
            visibility: None,
            resolution: None,
            meta: None,
            edges: None,
        };

        save_node(&file, &node).unwrap();
        let loaded = load_node(&file).unwrap();

        assert_eq!(loaded.id, "REQ-RT");
        assert_eq!(loaded.title, "Roundtrip Test");
        assert_eq!(loaded.priority, Some(crate::types::Priority::P0));
    }

    #[test]
    fn test_load_nodes_by_type_skips_bad_files() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let req_dir = root.join(LATTICE_DIR).join("requirements");
        fs::create_dir_all(&req_dir).unwrap();

        // Valid file
        fs::write(
            req_dir.join("good.yaml"),
            "id: REQ-GOOD\ntype: requirement\ntitle: Good\nbody: OK\nstatus: active\nversion: '1.0.0'\ncreated_at: '2026-01-01'\ncreated_by: test\n",
        ).unwrap();

        // Bad file
        fs::write(req_dir.join("bad.yaml"), "invalid yaml {{").unwrap();

        let nodes = load_nodes_by_type(root, "requirements").unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "REQ-GOOD");
    }

    #[test]
    fn test_load_nodes_by_type_empty_dir() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let req_dir = root.join(LATTICE_DIR).join("requirements");
        fs::create_dir_all(&req_dir).unwrap();

        let nodes = load_nodes_by_type(root, "requirements").unwrap();
        assert!(nodes.is_empty());
    }

    #[test]
    fn test_load_nodes_by_type_nonexistent_dir() {
        let dir = TempDir::new().unwrap();
        let nodes = load_nodes_by_type(dir.path(), "requirements").unwrap();
        assert!(nodes.is_empty());
    }

    #[test]
    fn test_find_node_path_finds_existing() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        init_lattice(root, false).unwrap();

        let req_dir = root.join(LATTICE_DIR).join("requirements");
        fs::write(
            req_dir.join("test.yaml"),
            "id: REQ-FIND\ntype: requirement\ntitle: Find Me\nbody: Body\nstatus: active\nversion: '1.0.0'\ncreated_at: '2026-01-01'\ncreated_by: test\n",
        ).unwrap();

        let path = find_node_path(root, "REQ-FIND").unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_find_node_path_returns_error_for_missing() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        init_lattice(root, false).unwrap();

        let result = find_node_path(root, "REQ-MISSING");
        assert!(result.is_err());
    }

    #[test]
    fn test_add_requirement_creates_file() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        init_lattice(root, false).unwrap();

        let options = AddRequirementOptions {
            id: "REQ-TEST-001".to_string(),
            title: "Test Requirement".to_string(),
            body: "Body text".to_string(),
            priority: crate::types::Priority::P0,
            category: "TEST".to_string(),
            tags: Some(vec!["tag1".to_string()]),
            derives_from: None,
            depends_on: None,
            status: crate::types::Status::Active,
            created_by: "test".to_string(),
        };

        let path = add_requirement(root, options).unwrap();
        assert!(path.exists());

        let node = load_node(&path).unwrap();
        assert_eq!(node.id, "REQ-TEST-001");
        assert_eq!(node.priority, Some(crate::types::Priority::P0));
    }

    #[test]
    fn test_add_requirement_rejects_duplicate_id() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        init_lattice(root, false).unwrap();

        let options = AddRequirementOptions {
            id: "REQ-DUP-001".to_string(),
            title: "First".to_string(),
            body: "Body".to_string(),
            priority: crate::types::Priority::P0,
            category: "TEST".to_string(),
            tags: None,
            derives_from: None,
            depends_on: None,
            status: crate::types::Status::Active,
            created_by: "test".to_string(),
        };
        add_requirement(root, options).unwrap();

        let duplicate = AddRequirementOptions {
            id: "REQ-DUP-001".to_string(),
            title: "Second with same ID".to_string(),
            body: "Body".to_string(),
            priority: crate::types::Priority::P1,
            category: "TEST".to_string(),
            tags: None,
            derives_from: None,
            depends_on: None,
            status: crate::types::Status::Active,
            created_by: "test".to_string(),
        };
        let result = add_requirement(root, duplicate);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_resolve_node_sets_resolution() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        init_lattice(root, false).unwrap();

        let options = AddRequirementOptions {
            id: "REQ-RES-001".to_string(),
            title: "Resolvable".to_string(),
            body: "Body".to_string(),
            priority: crate::types::Priority::P1,
            category: "TEST".to_string(),
            tags: None,
            derives_from: None,
            depends_on: None,
            status: crate::types::Status::Active,
            created_by: "test".to_string(),
        };
        add_requirement(root, options).unwrap();

        let resolve_opts = ResolveOptions {
            node_id: "REQ-RES-001".to_string(),
            resolution: crate::types::Resolution::Verified,
            reason: Some("Tests pass".to_string()),
            resolved_by: "test".to_string(),
        };
        let path = resolve_node(root, resolve_opts).unwrap();

        let node = load_node(&path).unwrap();
        assert!(node.resolution.is_some());
        assert_eq!(
            node.resolution.unwrap().status,
            crate::types::Resolution::Verified
        );
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World!", 20), "hello-world");
        assert_eq!(slugify("Test", 2), "te");
        assert_eq!(slugify("---test---", 20), "test");
    }

    #[test]
    fn test_refine_creates_sub_requirement() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        init_lattice(root, false).unwrap();

        // Create a parent requirement
        let parent_opts = AddRequirementOptions {
            id: "REQ-TEST-001".to_string(),
            title: "Parent Requirement".to_string(),
            body: "Parent body".to_string(),
            priority: crate::types::Priority::P0,
            category: "TEST".to_string(),
            tags: None,
            derives_from: Some(vec!["THX-001".to_string()]),
            depends_on: None,
            status: crate::types::Status::Active,
            created_by: "test".to_string(),
        };
        add_requirement(root, parent_opts).unwrap();

        // Refine it
        let refine_opts = RefineOptions {
            parent_id: "REQ-TEST-001".to_string(),
            gap_type: GapType::DesignDecision,
            title: "Error format choice".to_string(),
            description: "Need to decide JSON vs plain text errors".to_string(),
            proposed: Some("Use JSON with error code field".to_string()),
            implementation_id: None,
            created_by: "test".to_string(),
        };
        let result = refine_requirement(root, refine_opts).unwrap();

        assert_eq!(result.sub_requirement_id, "REQ-TEST-001-A");
        assert!(result.sub_requirement_path.exists());
        assert!(result.parent_updated);
        assert!(!result.implementation_updated);

        // Verify sub-requirement was created correctly
        let sub_node = load_node(&result.sub_requirement_path).unwrap();
        assert_eq!(sub_node.id, "REQ-TEST-001-A");
        assert_eq!(sub_node.status, crate::types::Status::Draft); // design_decision = draft
        assert!(sub_node.body.contains("Proposed Resolution"));
        assert!(
            sub_node
                .tags
                .as_ref()
                .unwrap()
                .contains(&"refinement".to_string())
        );

        // Verify parent now has depends_on edge to sub-requirement
        let parent_path = find_node_path(root, "REQ-TEST-001").unwrap();
        let parent = load_node(&parent_path).unwrap();
        let deps = parent.edges.unwrap().depends_on.unwrap();
        assert!(deps.iter().any(|e| e.target == "REQ-TEST-001-A"));
    }

    #[test]
    fn test_refine_clarification_auto_resolves() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        init_lattice(root, false).unwrap();

        let parent_opts = AddRequirementOptions {
            id: "REQ-CLR-001".to_string(),
            title: "Clarification Parent".to_string(),
            body: "Body".to_string(),
            priority: crate::types::Priority::P1,
            category: "TEST".to_string(),
            tags: None,
            derives_from: None,
            depends_on: None,
            status: crate::types::Status::Active,
            created_by: "test".to_string(),
        };
        add_requirement(root, parent_opts).unwrap();

        let refine_opts = RefineOptions {
            parent_id: "REQ-CLR-001".to_string(),
            gap_type: GapType::Clarification,
            title: "Use hyphens in slugs".to_string(),
            description: "Ambiguous separator character".to_string(),
            proposed: Some("Use hyphens, not underscores".to_string()),
            implementation_id: None,
            created_by: "test".to_string(),
        };
        let result = refine_requirement(root, refine_opts).unwrap();

        let sub = load_node(&result.sub_requirement_path).unwrap();
        assert_eq!(sub.status, crate::types::Status::Active); // clarification with proposal = active
    }

    #[test]
    fn test_refine_with_implementation_adds_gap_edge() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        init_lattice(root, false).unwrap();

        // Create parent requirement
        let parent_opts = AddRequirementOptions {
            id: "REQ-GAP-001".to_string(),
            title: "Gap Parent".to_string(),
            body: "Body".to_string(),
            priority: crate::types::Priority::P0,
            category: "TEST".to_string(),
            tags: None,
            derives_from: None,
            depends_on: None,
            status: crate::types::Status::Active,
            created_by: "test".to_string(),
        };
        add_requirement(root, parent_opts).unwrap();

        // Create implementation
        let impl_opts = AddImplementationOptions {
            id: "IMP-GAP-001".to_string(),
            title: "Gap Impl".to_string(),
            body: "Implementation".to_string(),
            language: Some("rust".to_string()),
            files: None,
            test_command: None,
            satisfies: Some(vec!["REQ-GAP-001".to_string()]),
            status: crate::types::Status::Active,
            created_by: "test".to_string(),
        };
        add_implementation(root, impl_opts).unwrap();

        // Refine with implementation reference
        let refine_opts = RefineOptions {
            parent_id: "REQ-GAP-001".to_string(),
            gap_type: GapType::MissingRequirement,
            title: "Concurrent write handling".to_string(),
            description: "No spec for concurrent writes".to_string(),
            proposed: None,
            implementation_id: Some("IMP-GAP-001".to_string()),
            created_by: "test".to_string(),
        };
        let result = refine_requirement(root, refine_opts).unwrap();

        assert!(result.implementation_updated);

        // Verify reveals_gap_in edge on implementation
        let impl_path = find_node_path(root, "IMP-GAP-001").unwrap();
        let impl_node = load_node(&impl_path).unwrap();
        let gaps = impl_node.edges.unwrap().reveals_gap_in.unwrap();
        assert!(gaps.iter().any(|e| e.target == "REQ-GAP-001"));
    }

    #[test]
    fn test_refine_sequential_suffixes() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        init_lattice(root, false).unwrap();

        let parent_opts = AddRequirementOptions {
            id: "REQ-SEQ-001".to_string(),
            title: "Sequential Parent".to_string(),
            body: "Body".to_string(),
            priority: crate::types::Priority::P0,
            category: "TEST".to_string(),
            tags: None,
            derives_from: None,
            depends_on: None,
            status: crate::types::Status::Active,
            created_by: "test".to_string(),
        };
        add_requirement(root, parent_opts).unwrap();

        // First refinement
        let r1 = refine_requirement(
            root,
            RefineOptions {
                parent_id: "REQ-SEQ-001".to_string(),
                gap_type: GapType::Clarification,
                title: "First gap".to_string(),
                description: "First".to_string(),
                proposed: Some("Answer".to_string()),
                implementation_id: None,
                created_by: "test".to_string(),
            },
        )
        .unwrap();
        assert_eq!(r1.sub_requirement_id, "REQ-SEQ-001-A");

        // Second refinement
        let r2 = refine_requirement(
            root,
            RefineOptions {
                parent_id: "REQ-SEQ-001".to_string(),
                gap_type: GapType::DesignDecision,
                title: "Second gap".to_string(),
                description: "Second".to_string(),
                proposed: None,
                implementation_id: None,
                created_by: "test".to_string(),
            },
        )
        .unwrap();
        assert_eq!(r2.sub_requirement_id, "REQ-SEQ-001-B");
    }

    #[test]
    fn test_gap_type_parse() {
        assert_eq!(
            "clarification".parse::<GapType>().unwrap(),
            GapType::Clarification
        );
        assert_eq!(
            "design_decision".parse::<GapType>().unwrap(),
            GapType::DesignDecision
        );
        assert_eq!(
            "missing_requirement".parse::<GapType>().unwrap(),
            GapType::MissingRequirement
        );
        assert_eq!(
            "contradiction".parse::<GapType>().unwrap(),
            GapType::Contradiction
        );
        assert!("invalid".parse::<GapType>().is_err());
    }

    #[test]
    fn test_parse_owner_repo_https() {
        let result = parse_owner_repo("https://github.com/forkzero/lattice.git");
        assert_eq!(
            result,
            Some(("forkzero".to_string(), "lattice".to_string()))
        );
    }

    #[test]
    fn test_parse_owner_repo_https_no_git() {
        let result = parse_owner_repo("https://github.com/forkzero/lattice");
        assert_eq!(
            result,
            Some(("forkzero".to_string(), "lattice".to_string()))
        );
    }

    #[test]
    fn test_parse_owner_repo_ssh() {
        let result = parse_owner_repo("git@github.com:forkzero/lattice.git");
        assert_eq!(
            result,
            Some(("forkzero".to_string(), "lattice".to_string()))
        );
    }

    #[test]
    fn test_parse_owner_repo_non_github() {
        let result = parse_owner_repo("https://gitlab.com/user/repo.git");
        assert_eq!(result, None);
    }

    #[test]
    fn test_refine_nonexistent_parent() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        init_lattice(root, false).unwrap();

        let result = refine_requirement(
            root,
            RefineOptions {
                parent_id: "REQ-MISSING-001".to_string(),
                gap_type: GapType::Clarification,
                title: "Test".to_string(),
                description: "Test".to_string(),
                proposed: None,
                implementation_id: None,
                created_by: "test".to_string(),
            },
        );
        assert!(result.is_err());
    }
}
