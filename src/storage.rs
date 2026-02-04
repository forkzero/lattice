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
