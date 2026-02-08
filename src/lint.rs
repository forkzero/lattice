//! Lint and validation for Lattice files.
//!
//! Linked requirements: REQ-CORE-012

use crate::storage::LATTICE_DIR;
use crate::types::{LatticeNode, NodeType};
use std::fmt;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Severity of a lint issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintSeverity {
    Error,
    Warning,
}

impl fmt::Display for LintSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LintSeverity::Error => write!(f, "error"),
            LintSeverity::Warning => write!(f, "warning"),
        }
    }
}

/// Whether a lint issue can be auto-fixed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fixable {
    Yes,
    No,
}

/// A single lint issue found in a file.
#[derive(Debug, Clone)]
pub struct LintIssue {
    pub file: PathBuf,
    pub node_id: Option<String>,
    pub severity: LintSeverity,
    pub message: String,
    pub fixable: Fixable,
}

impl fmt::Display for LintIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file_display = self
            .file
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| self.file.display().to_string());
        let node_part = self
            .node_id
            .as_ref()
            .map(|id| format!(" ({})", id))
            .unwrap_or_default();
        write!(
            f,
            "{}: {}{}: {}",
            self.severity, file_display, node_part, self.message
        )
    }
}

/// Result of linting the entire lattice.
#[derive(Debug, Clone)]
pub struct LintReport {
    pub issues: Vec<LintIssue>,
}

impl LintReport {
    pub fn errors(&self) -> Vec<&LintIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == LintSeverity::Error)
            .collect()
    }

    pub fn warnings(&self) -> Vec<&LintIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == LintSeverity::Warning)
            .collect()
    }

    pub fn fixable(&self) -> Vec<&LintIssue> {
        self.issues
            .iter()
            .filter(|i| i.fixable == Fixable::Yes)
            .collect()
    }

    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.severity == LintSeverity::Error)
    }
}

/// Lint all YAML files in the lattice directory.
pub fn lint_lattice(root: &Path) -> LintReport {
    let lattice_dir = root.join(LATTICE_DIR);
    let mut issues = Vec::new();

    if !lattice_dir.exists() {
        issues.push(LintIssue {
            file: lattice_dir,
            node_id: None,
            severity: LintSeverity::Error,
            message: "No .lattice directory found".to_string(),
            fixable: Fixable::No,
        });
        return LintReport { issues };
    }

    // Check config.yaml exists
    let config_path = lattice_dir.join("config.yaml");
    if !config_path.exists() {
        issues.push(LintIssue {
            file: config_path,
            node_id: None,
            severity: LintSeverity::Warning,
            message: "Missing config.yaml".to_string(),
            fixable: Fixable::Yes,
        });
    }

    // Validate all YAML files
    for entry in WalkDir::new(&lattice_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "yaml" || ext == "yml")
                .unwrap_or(false)
        })
    {
        let path = entry.path();

        // Skip config.yaml
        if path
            .file_name()
            .map(|f| f == "config.yaml")
            .unwrap_or(false)
            && path.parent() == Some(&lattice_dir)
        {
            continue;
        }

        lint_node_file(path, &lattice_dir, &mut issues);
    }

    // Check for duplicate IDs
    check_duplicate_ids(root, &mut issues);

    // Check edge references point to existing nodes
    check_edge_references(root, &mut issues);

    LintReport { issues }
}

/// Lint a single node YAML file.
fn lint_node_file(path: &Path, _lattice_dir: &Path, issues: &mut Vec<LintIssue>) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            issues.push(LintIssue {
                file: path.to_path_buf(),
                node_id: None,
                severity: LintSeverity::Error,
                message: "Cannot read file".to_string(),
                fixable: Fixable::No,
            });
            return;
        }
    };

    let node: LatticeNode = match serde_yaml::from_str(&content) {
        Ok(n) => n,
        Err(e) => {
            issues.push(LintIssue {
                file: path.to_path_buf(),
                node_id: None,
                severity: LintSeverity::Error,
                message: format!("Invalid YAML: {}", e),
                fixable: Fixable::No,
            });
            return;
        }
    };

    // Check required fields
    if node.id.is_empty() {
        issues.push(LintIssue {
            file: path.to_path_buf(),
            node_id: None,
            severity: LintSeverity::Error,
            message: "Missing or empty 'id' field".to_string(),
            fixable: Fixable::No,
        });
    }

    if node.title.is_empty() {
        issues.push(LintIssue {
            file: path.to_path_buf(),
            node_id: Some(node.id.clone()),
            severity: LintSeverity::Error,
            message: "Missing or empty 'title' field".to_string(),
            fixable: Fixable::No,
        });
    }

    if node.version.is_empty() {
        issues.push(LintIssue {
            file: path.to_path_buf(),
            node_id: Some(node.id.clone()),
            severity: LintSeverity::Warning,
            message: "Missing 'version' field (default: 1.0.0)".to_string(),
            fixable: Fixable::Yes,
        });
    } else {
        // Validate semver format
        let parts: Vec<&str> = node.version.split('.').collect();
        if parts.len() != 3 || parts.iter().any(|p| p.parse::<u64>().is_err()) {
            issues.push(LintIssue {
                file: path.to_path_buf(),
                node_id: Some(node.id.clone()),
                severity: LintSeverity::Warning,
                message: format!("Invalid semver version: '{}'", node.version),
                fixable: Fixable::No,
            });
        }
    }

    // Check node type matches directory
    let expected_dir = match node.node_type {
        NodeType::Source => "sources",
        NodeType::Thesis => "theses",
        NodeType::Requirement => "requirements",
        NodeType::Implementation => "implementations",
    };

    let in_correct_dir = path.components().any(|c| c.as_os_str() == expected_dir);

    if !in_correct_dir {
        issues.push(LintIssue {
            file: path.to_path_buf(),
            node_id: Some(node.id.clone()),
            severity: LintSeverity::Warning,
            message: format!(
                "Node type '{:?}' should be in '{}/' directory",
                node.node_type, expected_dir
            ),
            fixable: Fixable::No,
        });
    }

    // Requirements should have priority
    if node.node_type == NodeType::Requirement && node.priority.is_none() {
        issues.push(LintIssue {
            file: path.to_path_buf(),
            node_id: Some(node.id.clone()),
            severity: LintSeverity::Warning,
            message: "Requirement missing 'priority' field".to_string(),
            fixable: Fixable::No,
        });
    }

    // Check edge version bindings
    if let Some(edges) = &node.edges {
        let check_edges = |refs: &Option<Vec<crate::types::EdgeReference>>,
                           edge_type: &str,
                           issues: &mut Vec<LintIssue>| {
            if let Some(edge_refs) = refs {
                for edge_ref in edge_refs {
                    if edge_ref.target.is_empty() {
                        issues.push(LintIssue {
                            file: path.to_path_buf(),
                            node_id: Some(node.id.clone()),
                            severity: LintSeverity::Error,
                            message: format!("Empty target in '{}' edge", edge_type),
                            fixable: Fixable::No,
                        });
                    }
                    if edge_ref.version.is_none() {
                        issues.push(LintIssue {
                            file: path.to_path_buf(),
                            node_id: Some(node.id.clone()),
                            severity: LintSeverity::Warning,
                            message: format!(
                                "Edge '{}' -> '{}' missing version binding",
                                edge_type, edge_ref.target
                            ),
                            fixable: Fixable::Yes,
                        });
                    }
                }
            }
        };

        check_edges(&edges.supported_by, "supported_by", issues);
        check_edges(&edges.derives_from, "derives_from", issues);
        check_edges(&edges.depends_on, "depends_on", issues);
        check_edges(&edges.satisfies, "satisfies", issues);
        check_edges(&edges.extends, "extends", issues);
        check_edges(&edges.reveals_gap_in, "reveals_gap_in", issues);
        check_edges(&edges.challenges, "challenges", issues);
        check_edges(&edges.validates, "validates", issues);
        check_edges(&edges.conflicts_with, "conflicts_with", issues);
        check_edges(&edges.supersedes, "supersedes", issues);
    }
}

/// Check for duplicate node IDs across all files.
fn check_duplicate_ids(root: &Path, issues: &mut Vec<LintIssue>) {
    let mut seen: std::collections::HashMap<String, PathBuf> = std::collections::HashMap::new();

    let lattice_dir = root.join(LATTICE_DIR);
    for entry in WalkDir::new(&lattice_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "yaml" || ext == "yml")
                .unwrap_or(false)
        })
    {
        let path = entry.path();
        if path
            .file_name()
            .map(|f| f == "config.yaml")
            .unwrap_or(false)
            && path.parent() == Some(&lattice_dir)
        {
            continue;
        }

        if let Ok(node) = crate::storage::load_node(path) {
            if let Some(prev_path) = seen.get(&node.id) {
                issues.push(LintIssue {
                    file: path.to_path_buf(),
                    node_id: Some(node.id.clone()),
                    severity: LintSeverity::Error,
                    message: format!(
                        "Duplicate ID '{}' (also in {})",
                        node.id,
                        prev_path.display()
                    ),
                    fixable: Fixable::No,
                });
            } else {
                seen.insert(node.id, path.to_path_buf());
            }
        }
    }
}

/// Check that edge references point to existing node IDs.
fn check_edge_references(root: &Path, issues: &mut Vec<LintIssue>) {
    let index = match crate::graph::build_node_index(root) {
        Ok(idx) => idx,
        Err(_) => return,
    };

    for node in index.values() {
        for edge_ref in node.all_edges() {
            if !index.contains_key(&edge_ref.target) {
                issues.push(LintIssue {
                    file: PathBuf::from(format!("<{}>", node.id)),
                    node_id: Some(node.id.clone()),
                    severity: LintSeverity::Warning,
                    message: format!("Edge references non-existent node '{}'", edge_ref.target),
                    fixable: Fixable::No,
                });
            }
        }
    }
}

/// Apply auto-fixes for fixable issues.
pub fn fix_issues(root: &Path, report: &LintReport) -> Vec<String> {
    let mut fixed = Vec::new();
    let lattice_dir = root.join(LATTICE_DIR);

    for issue in report.fixable() {
        // Fix missing config.yaml
        if issue.message == "Missing config.yaml" {
            let config_path = lattice_dir.join("config.yaml");
            if std::fs::write(&config_path, crate::storage::DEFAULT_CONFIG).is_ok() {
                fixed.push(format!("Created {}", config_path.display()));
            }
            continue;
        }

        // Fix missing version on edges
        if issue.message.contains("missing version binding")
            && let Some(node_id) = &issue.node_id
            && let Ok(path) = crate::storage::find_node_path(root, node_id)
            && let Ok(mut node) = crate::storage::load_node(&path)
        {
            let mut modified = false;
            if let Some(edges) = &mut node.edges {
                let fix_refs = |refs: &mut Option<Vec<crate::types::EdgeReference>>| -> bool {
                    let mut changed = false;
                    if let Some(edge_refs) = refs {
                        for edge_ref in edge_refs.iter_mut() {
                            if edge_ref.version.is_none() {
                                edge_ref.version = Some("1.0.0".to_string());
                                changed = true;
                            }
                        }
                    }
                    changed
                };
                modified |= fix_refs(&mut edges.supported_by);
                modified |= fix_refs(&mut edges.derives_from);
                modified |= fix_refs(&mut edges.depends_on);
                modified |= fix_refs(&mut edges.satisfies);
                modified |= fix_refs(&mut edges.extends);
                modified |= fix_refs(&mut edges.reveals_gap_in);
                modified |= fix_refs(&mut edges.challenges);
                modified |= fix_refs(&mut edges.validates);
                modified |= fix_refs(&mut edges.conflicts_with);
                modified |= fix_refs(&mut edges.supersedes);
            }
            if modified && crate::storage::save_node(&path, &node).is_ok() {
                fixed.push(format!("Added version binding to edges in {}", node_id));
            }
        }
    }

    fixed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_lint_no_lattice_dir() {
        let dir = TempDir::new().unwrap();
        let report = lint_lattice(dir.path());
        assert!(report.has_errors());
        assert_eq!(report.errors().len(), 1);
        assert!(report.errors()[0].message.contains("No .lattice directory"));
    }

    #[test]
    fn test_lint_valid_lattice() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        crate::storage::init_lattice(root, false).unwrap();

        let req_dir = root.join(LATTICE_DIR).join("requirements");
        fs::write(
            req_dir.join("test.yaml"),
            "id: REQ-001\ntype: requirement\ntitle: Test\nbody: Body\nstatus: active\nversion: '1.0.0'\ncreated_at: '2026-01-01'\ncreated_by: test\npriority: P0\n",
        ).unwrap();

        let report = lint_lattice(root);
        assert!(!report.has_errors());
    }

    #[test]
    fn test_lint_invalid_yaml() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        crate::storage::init_lattice(root, false).unwrap();

        let req_dir = root.join(LATTICE_DIR).join("requirements");
        fs::write(req_dir.join("bad.yaml"), "{{invalid").unwrap();

        let report = lint_lattice(root);
        assert!(report.has_errors());
        assert!(report.errors()[0].message.contains("Invalid YAML"));
    }

    #[test]
    fn test_lint_missing_priority_on_requirement() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        crate::storage::init_lattice(root, false).unwrap();

        let req_dir = root.join(LATTICE_DIR).join("requirements");
        fs::write(
            req_dir.join("nopri.yaml"),
            "id: REQ-NP\ntype: requirement\ntitle: No Priority\nbody: Body\nstatus: active\nversion: '1.0.0'\ncreated_at: '2026-01-01'\ncreated_by: test\n",
        ).unwrap();

        let report = lint_lattice(root);
        let warnings = report.warnings();
        assert!(warnings.iter().any(|w| w.message.contains("priority")));
    }

    #[test]
    fn test_lint_duplicate_ids() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        crate::storage::init_lattice(root, false).unwrap();

        let req_dir = root.join(LATTICE_DIR).join("requirements");
        let yaml = "id: REQ-DUP\ntype: requirement\ntitle: Dup\nbody: Body\nstatus: active\nversion: '1.0.0'\ncreated_at: '2026-01-01'\ncreated_by: test\npriority: P0\n";
        fs::write(req_dir.join("dup1.yaml"), yaml).unwrap();
        fs::write(req_dir.join("dup2.yaml"), yaml).unwrap();

        let report = lint_lattice(root);
        assert!(report.has_errors());
        assert!(
            report
                .errors()
                .iter()
                .any(|e| e.message.contains("Duplicate ID"))
        );
    }

    #[test]
    fn test_lint_dangling_edge_reference() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        crate::storage::init_lattice(root, false).unwrap();

        let req_dir = root.join(LATTICE_DIR).join("requirements");
        fs::write(
            req_dir.join("dangling.yaml"),
            "id: REQ-DANG\ntype: requirement\ntitle: Dangling\nbody: Body\nstatus: active\nversion: '1.0.0'\ncreated_at: '2026-01-01'\ncreated_by: test\npriority: P0\nedges:\n  depends_on:\n    - target: REQ-NONEXISTENT\n      version: '1.0.0'\n",
        ).unwrap();

        let report = lint_lattice(root);
        let warnings = report.warnings();
        assert!(warnings.iter().any(|w| w.message.contains("non-existent")));
    }

    #[test]
    fn test_fix_creates_config() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let lattice_dir = root.join(LATTICE_DIR);
        fs::create_dir_all(lattice_dir.join("requirements")).unwrap();
        // No config.yaml

        let report = lint_lattice(root);
        assert!(
            report
                .warnings()
                .iter()
                .any(|w| w.message.contains("config.yaml"))
        );

        let fixed = fix_issues(root, &report);
        assert!(!fixed.is_empty());
        assert!(lattice_dir.join("config.yaml").exists());
    }

    #[test]
    fn test_lint_report_display() {
        let issue = LintIssue {
            file: PathBuf::from("test.yaml"),
            node_id: Some("REQ-001".to_string()),
            severity: LintSeverity::Error,
            message: "test error".to_string(),
            fixable: Fixable::No,
        };
        let display = format!("{}", issue);
        assert!(display.contains("error"));
        assert!(display.contains("REQ-001"));
    }
}
