//! Lattice - A knowledge coordination protocol for human-agent collaboration.
//!
//! This library provides the core functionality for managing a lattice of
//! interconnected knowledge nodes (sources, theses, requirements, implementations)
//! with version-bound edges and drift detection.

pub mod export;
pub mod graph;
pub mod html_export;
pub mod lint;
pub mod mcp;
pub mod storage;
pub mod types;

pub use export::{Audience, ExportOptions, LatticeData, export_narrative};
pub use graph::{
    DriftReport, DriftSeverity, Plan, PlannedItem, build_node_index, find_drift, generate_plan,
};
pub use html_export::{HtmlExportOptions, export_html};
pub use lint::{LintReport, LintSeverity, fix_issues, lint_lattice};
pub use storage::{
    AddImplementationOptions, AddRequirementOptions, AddSourceOptions, AddThesisOptions,
    LATTICE_DIR, ResolveOptions, VerifyOptions, add_implementation, add_requirement, add_source,
    add_thesis, find_lattice_root, find_node_path, get_git_user, init_lattice, load_all_nodes,
    load_nodes_by_type, resolve_node, verify_implementation,
};
pub use types::{LatticeNode, NodeIndex, NodeType, Priority, Resolution, ResolutionInfo, Status};
