//! Lattice - A knowledge coordination protocol for human-agent collaboration.
//!
//! This library provides the core functionality for managing a lattice of
//! interconnected knowledge nodes (sources, theses, requirements, implementations)
//! with version-bound edges and drift detection.

pub mod diff;
pub mod export;
pub mod graph;
pub mod html_export;
pub mod lint;
pub mod mcp;
pub mod push;
pub mod search;
pub mod storage;
pub mod types;
pub mod update;

pub use diff::{
    ChangeType, DiffEntry, DiffError, DiffResult, format_diff_markdown, format_entry_text,
    lattice_diff,
};
pub use export::{Audience, ExportOptions, LatticeData, export_narrative};
pub use graph::{
    DriftReport, DriftSeverity, Plan, PlannedItem, build_node_index, find_drift, generate_plan,
};
pub use html_export::{HtmlExportOptions, export_html};
pub use lint::{LintReport, LintSeverity, fix_issues, lint_lattice};
#[cfg(feature = "vector-search")]
pub use search::FastEmbedProvider;
pub use search::{
    EmbeddingProvider, IndexStatus, SearchEngine, SearchIndex, SearchParams, SearchResult,
    SearchResults, split_csv,
};
pub use storage::{
    AddEdgeOptions, AddImplementationOptions, AddRequirementOptions, AddSourceOptions,
    AddThesisOptions, EDGE_TYPES, EditNodeOptions, GapType, LATTICE_DIR, LatticeConfig,
    RefineOptions, RefineResult, RemoveEdgeOptions, ReplaceEdgeOptions, ResolveOptions,
    VerifyOptions, add_edge, add_implementation, add_requirement, add_source, add_thesis,
    edit_node, find_lattice_root, find_node_path, get_git_user, get_github_pages_url, init_lattice,
    load_all_nodes, load_config, load_nodes_by_type, refine_requirement, remove_edge, replace_edge,
    resolve_node, verify_implementation,
};
pub use types::{LatticeNode, NodeIndex, NodeType, Priority, Resolution, ResolutionInfo, Status};
