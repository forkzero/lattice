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
pub const DEFAULT_CONFIG: &str = r#"# Lattice configuration
version: "1.0"
"#;

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

    // Create config.yaml
    let config_path = lattice_dir.join("config.yaml");
    fs::write(&config_path, DEFAULT_CONFIG)?;
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

/// Add an implementation to the lattice.
pub fn add_implementation(
    root: &Path,
    options: AddImplementationOptions,
) -> Result<PathBuf, StorageError> {
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
}
