//! Lattice CLI - Command-line interface for Lattice operations.
//!
//! Linked requirements: REQ-CLI-001 through REQ-CLI-005, REQ-CORE-009

use clap::{Parser, Subcommand};
use colored::Colorize;
use lattice::{
    AddEdgeOptions, AddImplementationOptions, AddMessageOptions, AddRequirementOptions,
    AddSourceOptions, AddThesisOptions, Audience, CURRENT_SCHEMA_VERSION, DiffEntry, DriftSeverity,
    EditNodeOptions, ExportOptions, GapType, HtmlExportOptions, LATTICE_DIR, LatticeData,
    LintSeverity, NodeMeta, NodeType, Plan, Priority, RefineOptions, RemoveEdgeOptions,
    ReplaceEdgeOptions, Resolution, ResolveOptions, SchemaCheck, SearchEngine, SearchParams,
    Status, VerifyOptions, add_edge, add_implementation, add_message, add_requirement, add_source,
    add_thesis, build_node_index, check_schema_version, edit_node, export_html, export_narrative,
    find_drift, find_lattice_root, find_node_path, fix_issues, format_diff_markdown,
    format_entry_text, generate_plan, get_git_user, get_github_pages_url, init_lattice,
    lattice_diff, lint_lattice, load_all_nodes, load_config, load_nodes_by_type,
    refine_requirement, remove_edge, replace_edge, resolve_node, split_csv, verify_implementation,
};
use serde_json::json;
use std::env;
use std::fs;
use std::process;

#[derive(Parser)]
#[command(name = "lattice")]
#[command(
    about = "A knowledge coordination protocol for human-agent collaboration.\nDesigned to be discoverable by LLMs — try: lattice --json"
)]
#[command(version)]
#[command(disable_help_subcommand = true)]
struct Cli {
    /// Output machine-readable command catalog as JSON
    #[arg(long)]
    json: bool,

    /// With --json: output compact schema only (command signatures, no examples/descriptions)
    #[arg(long, requires = "json")]
    compact: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    // ── Knowledge Graph ─────────────────────────────────────────────
    /// Add a source, thesis, requirement, implementation, or edge
    Add {
        #[command(subcommand)]
        add_command: AddCommands,
    },

    /// Get a node by ID (SRC-*, THX-*, REQ-*, IMP-*)
    Get {
        /// Node ID
        id: String,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// List sources, theses, requirements, or implementations
    List {
        /// Node type (sources, theses, requirements, implementations)
        node_type: String,

        /// Filter by workflow status (draft, active, deprecated, superseded)
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by priority (P0, P1, P2) — requirements only
        #[arg(short, long)]
        priority: Option<String>,

        /// Show only blocked items (resolution filter)
        #[arg(long)]
        blocked: bool,

        /// Show only deferred items (resolution filter)
        #[arg(long)]
        deferred: bool,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Search sources, theses, requirements, or implementations
    Search {
        /// Node type to search (sources, theses, requirements, implementations)
        #[arg(value_name = "NODE_TYPE")]
        positional_type: Option<String>,

        /// Node type to search (alternative to positional arg)
        #[arg(short = 't', long)]
        node_type: Option<String>,

        /// Text search in title and body
        #[arg(short, long)]
        query: Option<String>,

        /// Filter by priority (P0, P1, P2)
        #[arg(short, long)]
        priority: Option<String>,

        /// Filter by resolution (verified, blocked, deferred, wontfix, unresolved)
        #[arg(short, long)]
        resolution: Option<String>,

        /// Filter by tag (single tag)
        #[arg(long)]
        tag: Option<String>,

        /// Filter by multiple tags (comma-separated, all must match)
        #[arg(long)]
        tags: Option<String>,

        /// Filter by category
        #[arg(short, long)]
        category: Option<String>,

        /// Filter by ID prefix
        #[arg(long)]
        id_prefix: Option<String>,

        /// Find nodes related to this node ID (graph proximity)
        #[arg(long)]
        related_to: Option<String>,

        /// Build or rebuild the search index
        #[arg(long)]
        index: bool,

        /// Show search index health (indexed, stale, missing counts)
        #[arg(long)]
        index_status: bool,

        /// Use semantic (vector) search instead of keyword matching (requires vector-search feature and index)
        #[cfg(feature = "vector-search")]
        #[arg(long)]
        semantic: bool,

        /// Minimum score threshold (filters results below this score)
        #[arg(long)]
        min_score: Option<f32>,

        /// Maximum number of results to return
        #[arg(long)]
        limit: Option<usize>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Edit a source, thesis, requirement, or implementation
    Edit {
        /// Node ID (e.g., REQ-CORE-001, IMP-DIST-001)
        id: String,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New body
        #[arg(long)]
        body: Option<String>,

        /// New status (draft, active, deprecated, superseded)
        #[arg(long)]
        status: Option<String>,

        /// New priority (P0, P1, P2) — requirements only
        #[arg(long)]
        priority: Option<String>,

        /// Comma-separated tags (replaces existing)
        #[arg(long)]
        tags: Option<String>,

        /// New category
        #[arg(long)]
        category: Option<String>,

        /// Comma-separated file paths (replaces existing) — implementations only
        #[arg(long)]
        files: Option<String>,

        /// Test command (e.g., cargo test) — implementations only
        #[arg(long)]
        test_command: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Resolve a requirement (verified, blocked, deferred, wontfix)
    Resolve {
        /// Requirement ID (e.g., REQ-CICD-003)
        id: String,

        /// Mark as verified (requirement satisfied)
        #[arg(long, conflicts_with_all = ["blocked", "deferred", "wontfix"])]
        verified: bool,

        /// Mark as blocked with reason (external constraint)
        #[arg(long, conflicts_with_all = ["verified", "deferred", "wontfix"])]
        blocked: Option<String>,

        /// Mark as deferred with reason (user choice to postpone)
        #[arg(long, conflicts_with_all = ["verified", "blocked", "wontfix"])]
        deferred: Option<String>,

        /// Mark as wontfix with reason (will not implement)
        #[arg(long, conflicts_with_all = ["verified", "blocked", "deferred"])]
        wontfix: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Record that an implementation satisfies a requirement
    Verify {
        /// Implementation ID (e.g., IMP-STORAGE-001)
        implementation: String,

        /// Must be the literal word "satisfies"
        #[arg(value_parser = clap::builder::PossibleValuesParser::new(["satisfies"]))]
        relation: String,

        /// Requirement ID (e.g., REQ-CORE-001)
        requirement: String,

        /// Record that tests pass
        #[arg(long)]
        tests_pass: bool,

        /// Record coverage percentage (0.0-1.0)
        #[arg(long)]
        coverage: Option<f64>,

        /// Comma-separated file paths as evidence
        #[arg(long)]
        files: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Create a sub-requirement from a gap found during implementation
    Refine {
        /// Parent requirement ID (e.g., REQ-CORE-005)
        parent: String,

        /// Gap type: clarification, design_decision, missing_requirement, contradiction
        #[arg(long)]
        gap_type: String,

        /// Brief title for the sub-requirement
        #[arg(long)]
        title: String,

        /// What is underspecified and why it matters
        #[arg(long)]
        description: String,

        /// Proposed resolution
        #[arg(long, alias = "proposal")]
        proposed: Option<String>,

        /// Implementation ID that discovered this gap
        #[arg(long, alias = "discovered-by")]
        implementation: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Remove an edge between two nodes
    Remove {
        #[command(subcommand)]
        remove_command: RemoveCommands,
    },

    /// Retarget an edge to a different node
    Replace {
        #[command(subcommand)]
        replace_command: ReplaceCommands,
    },

    // ── Health & Analysis ───────────────────────────────────────────
    /// Status overview — node counts, resolution, drift, orphans
    Summary {
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Check for version drift in edge bindings
    Drift {
        /// Exit with non-zero status if drift detected
        #[arg(long)]
        check: bool,

        /// Acknowledge drift on a node (re-snapshot edge versions)
        #[arg(long)]
        acknowledge: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Check if lattice is updated alongside code changes
    Freshness {
        /// Maximum allowed age gap between code and lattice changes, in hours (default: 72)
        #[arg(long, default_value = "72")]
        threshold: u64,

        /// Exit with code 2 if lattice is stale (for CI/hooks)
        #[arg(long)]
        check: bool,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Assess change pressure — how much has shifted and what's affected
    Assess {
        /// Exit with code 2 if change pressure exceeds threshold
        #[arg(long)]
        check: bool,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Unified health check — freshness, change pressure, and code impact in one verdict
    Health {
        /// Exit with code 2 on FAIL, code 0 on PASS/WARN
        #[arg(long)]
        check: bool,

        /// Also run lint — any lint issues escalate verdict to FAIL
        #[arg(long)]
        strict: bool,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Check lattice files for structural issues
    Lint {
        /// Attempt to auto-fix fixable issues
        #[arg(long)]
        fix: bool,

        /// Exit with non-zero status on any issue (for CI)
        #[arg(long)]
        strict: bool,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show changes to sources, theses, requirements since a git ref
    Diff {
        /// Git ref to compare against (default: merge-base with main)
        #[arg(long, conflicts_with = "since_push")]
        since: Option<String>,

        /// Use the last push SHA as the baseline (fetches from API)
        #[arg(long, conflicts_with = "since")]
        since_push: bool,

        /// API URL for --since-push (overrides config and LATTICE_API_URL env var)
        #[arg(long)]
        api_url: Option<String>,

        /// API key for --since-push (overrides config and LATTICE_API_KEY env var)
        #[arg(long)]
        api_key: Option<String>,

        /// Output as markdown (for GitHub comments)
        #[arg(long)]
        md: bool,

        /// Show raw git diff of .lattice/ files
        #[arg(long)]
        raw: bool,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Plan implementation order based on requirement dependencies
    Plan {
        /// Requirement IDs to plan (e.g., REQ-CLI-001 REQ-CLI-002)
        #[arg(required = true)]
        requirements: Vec<String>,
    },

    /// Export to narrative, JSON, HTML, or GitHub Pages
    Export {
        /// Export format (narrative, json, html, pages)
        #[arg(short, long, default_value = "narrative")]
        format: String,

        /// Target audience for narrative (investor, contributor, overview)
        #[arg(short, long, default_value = "overview")]
        audience: String,

        /// Document title
        #[arg(short, long, default_value = "Lattice")]
        title: String,

        /// Include nodes marked as internal
        #[arg(long)]
        include_internal: bool,

        /// Output directory for HTML export
        #[arg(short, long)]
        output: Option<String>,
    },

    // ── Setup ───────────────────────────────────────────────────────
    /// Initialize a new lattice in the current directory
    Init {
        /// Overwrite existing lattice
        #[arg(short, long)]
        force: bool,

        /// Also install agent definitions (.claude/agents/)
        #[arg(long)]
        agents: bool,

        /// Install Claude Code skill and agent definitions
        #[arg(long)]
        skill: bool,
    },

    /// Self-update to the latest version
    Update {
        /// Only check for updates, don't install
        #[arg(long)]
        check: bool,

        /// Force update even if already up to date
        #[arg(long)]
        force: bool,

        /// Install a specific version (e.g., 0.1.5 or v0.1.5)
        #[arg(long)]
        version: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show commands, concepts, or workflows
    Help {
        /// Output as structured JSON for agent consumption
        #[arg(long)]
        json: bool,

        /// With --json: output compact schema only (command signatures, no examples/descriptions)
        #[arg(long, requires = "json")]
        compact: bool,

        /// Topic to show: concepts, workflows (omit for command list)
        topic: Option<String>,
    },

    // ── Integrations (hidden from --help) ───────────────────────────
    /// Run as MCP server over stdio
    #[command(hide = true)]
    Mcp,

    /// Output CLAUDE.md integration snippet
    #[command(hide = true)]
    Prompt {
        /// Output MCP version instead of CLI version
        #[arg(long)]
        mcp: bool,
    },

    /// Push lattice data to a remote API
    #[command(hide = true)]
    Push {
        /// API URL (overrides config and LATTICE_API_URL env var)
        #[arg(long)]
        api_url: Option<String>,

        /// API key (overrides config and LATTICE_API_KEY env var)
        #[arg(long)]
        api_key: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Migrate existing .lattice/ to v0.2.0 schema
    #[command(hide = true)]
    Migrate,
}

#[derive(Subcommand)]
enum AddCommands {
    /// Add a requirement (testable specification derived from theses)
    Requirement {
        /// Requirement ID (e.g., REQ-API-003)
        #[arg(long)]
        id: String,

        /// Requirement title
        #[arg(long)]
        title: String,

        /// Requirement body/description
        #[arg(long)]
        body: String,

        /// Priority (P0, P1, P2)
        #[arg(long)]
        priority: String,

        /// Category (e.g., API, CORE, CLI)
        #[arg(long)]
        category: String,

        /// Comma-separated tags
        #[arg(long)]
        tags: Option<String>,

        /// Comma-separated thesis IDs this derives from
        #[arg(long)]
        derives_from: Option<String>,

        /// Comma-separated requirement IDs this depends on
        #[arg(long)]
        depends_on: Option<String>,

        /// Status (draft, active)
        #[arg(long, default_value = "active")]
        status: String,

        /// Author (e.g., human:george, agent:claude)
        #[arg(long)]
        created_by: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Add a thesis (strategic claim backed by sources)
    Thesis {
        /// Thesis ID (e.g., THX-AGENT-PROTOCOL)
        #[arg(long)]
        id: String,

        /// Thesis title
        #[arg(long)]
        title: String,

        /// Thesis body/description
        #[arg(long)]
        body: String,

        /// Category (value_prop, market, technical, risk, competitive)
        #[arg(long)]
        category: String,

        /// Confidence level 0.0-1.0
        #[arg(long, default_value = "0.8")]
        confidence: f64,

        /// Comma-separated source IDs
        #[arg(long)]
        supported_by: Option<String>,

        /// Status (draft, active)
        #[arg(long, default_value = "active")]
        status: String,

        /// Author
        #[arg(long)]
        created_by: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Add a source (research, paper, or reference material)
    Source {
        /// Source ID (e.g., SRC-JSON-RPC)
        #[arg(long)]
        id: String,

        /// Source title
        #[arg(long)]
        title: String,

        /// Source body/description
        #[arg(long)]
        body: String,

        /// Source URL
        #[arg(long)]
        url: Option<String>,

        /// Comma-separated citations
        #[arg(long)]
        citations: Option<String>,

        /// Reliability (peer_reviewed, industry, blog, unverified)
        #[arg(long, default_value = "unverified")]
        reliability: String,

        /// Status (draft, active)
        #[arg(long, default_value = "active")]
        status: String,

        /// Author
        #[arg(long)]
        created_by: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Add an edge (typed, version-bound link between two nodes)
    Edge {
        /// Source node ID (edge goes FROM this node)
        #[arg(long)]
        from: String,

        /// Edge type (supported_by, derives_from, depends_on, satisfies, extends,
        /// reveals_gap_in, challenges, validates, conflicts_with, supersedes)
        #[arg(long, alias = "type", name = "type")]
        edge_type: String,

        /// Target node ID (edge goes TO this node)
        #[arg(long)]
        to: String,

        /// Why this edge exists
        #[arg(long)]
        rationale: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Add an implementation (code that satisfies requirements)
    Implementation {
        /// Implementation ID (e.g., IMP-STORAGE-001)
        #[arg(long)]
        id: String,

        /// Implementation title
        #[arg(long)]
        title: String,

        /// Implementation body/description
        #[arg(long)]
        body: String,

        /// Programming language (e.g., rust, python)
        #[arg(long)]
        language: Option<String>,

        /// Comma-separated file paths
        #[arg(long)]
        files: Option<String>,

        /// Test command (e.g., cargo test)
        #[arg(long)]
        test_command: Option<String>,

        /// Comma-separated requirement IDs this satisfies
        #[arg(long)]
        satisfies: Option<String>,

        /// Status (draft, active)
        #[arg(long, default_value = "active")]
        status: String,

        /// Author
        #[arg(long)]
        created_by: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Add a message (persona-specific claim grounded in theses)
    Message {
        /// Message ID (e.g., MSG-DEV-PERF-001)
        #[arg(long)]
        id: String,

        /// Message title
        #[arg(long)]
        title: String,

        /// Message body (the actual claim or talking point)
        #[arg(long)]
        body: String,

        /// Target persona (e.g., developer, investor, operator)
        #[arg(long)]
        persona: String,

        /// Comma-separated channels (e.g., docs, landing-page, pitch-deck)
        #[arg(long)]
        channel: Option<String>,

        /// Comma-separated thesis IDs this message is grounded in
        #[arg(long)]
        grounded_in: Option<String>,

        /// Comma-separated tags
        #[arg(long)]
        tags: Option<String>,

        /// Author
        #[arg(long)]
        created_by: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum RemoveCommands {
    /// Remove an edge between two nodes
    Edge {
        /// Source node ID (edge goes FROM this node)
        #[arg(long)]
        from: String,

        /// Edge type (supported_by, derives_from, depends_on, satisfies, extends,
        /// reveals_gap_in, challenges, validates, conflicts_with, supersedes)
        #[arg(long, alias = "type", name = "type")]
        edge_type: String,

        /// Target node ID (edge goes TO this node)
        #[arg(long)]
        to: String,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum ReplaceCommands {
    /// Replace an edge's target with a new target
    Edge {
        /// Source node ID (edge goes FROM this node)
        #[arg(long)]
        from: String,

        /// Edge type (supported_by, derives_from, depends_on, satisfies, extends,
        /// reveals_gap_in, challenges, validates, conflicts_with, supersedes)
        #[arg(long, alias = "type", name = "type")]
        edge_type: String,

        /// Current target node ID to replace
        #[arg(long)]
        old_to: String,

        /// New target node ID
        #[arg(long)]
        new_to: String,

        /// Why this edge exists (updates existing rationale)
        #[arg(long)]
        rationale: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

fn get_lattice_root() -> std::path::PathBuf {
    let cwd = env::current_dir().expect("Failed to get current directory");
    match find_lattice_root(&cwd) {
        Some(root) => root,
        None => {
            eprintln!("{}", "Not in a lattice directory".red());
            process::exit(1);
        }
    }
}

fn parse_priority(s: &str) -> Priority {
    match s.to_uppercase().as_str() {
        "P0" => Priority::P0,
        "P1" => Priority::P1,
        "P2" => Priority::P2,
        _ => {
            eprintln!(
                "{}",
                format!("Invalid priority: {}. Must be P0, P1, or P2", s).red()
            );
            process::exit(1);
        }
    }
}

fn parse_status(s: &str) -> Status {
    match s.to_lowercase().as_str() {
        "draft" => Status::Draft,
        "active" => Status::Active,
        "deprecated" => Status::Deprecated,
        "superseded" => Status::Superseded,
        _ => {
            eprintln!(
                "{}",
                format!(
                    "Invalid status: {}. Must be draft, active, deprecated, or superseded",
                    s
                )
                .red()
            );
            process::exit(1);
        }
    }
}

fn parse_reliability(s: &str) -> lattice::types::Reliability {
    match s.to_lowercase().as_str() {
        "peer_reviewed" => lattice::types::Reliability::PeerReviewed,
        "industry" => lattice::types::Reliability::Industry,
        "blog" => lattice::types::Reliability::Blog,
        "unverified" => lattice::types::Reliability::Unverified,
        _ => {
            eprintln!(
                "{}",
                format!(
                    "Invalid reliability: {}. Must be peer_reviewed, industry, blog, or unverified",
                    s
                )
                .red()
            );
            process::exit(1);
        }
    }
}

fn parse_thesis_category(s: &str) -> lattice::types::ThesisCategory {
    match s.to_lowercase().as_str() {
        "value_prop" => lattice::types::ThesisCategory::ValueProp,
        "market" => lattice::types::ThesisCategory::Market,
        "technical" => lattice::types::ThesisCategory::Technical,
        "risk" => lattice::types::ThesisCategory::Risk,
        "competitive" => lattice::types::ThesisCategory::Competitive,
        _ => {
            eprintln!(
                "{}",
                format!(
                    "Invalid category: {}. Must be value_prop, market, technical, risk, or competitive",
                    s
                )
                .red()
            );
            process::exit(1);
        }
    }
}

fn is_json(format: &str) -> bool {
    format == "json"
}

fn emit_created(format: &str, node_type: &str, id: &str, path: &std::path::Path) {
    if is_json(format) {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "success": true,
                "type": node_type,
                "id": id,
                "file": path.display().to_string(),
            }))
            .unwrap()
        );
    } else {
        println!("{}", format!("Created {}: {}", node_type, id).green());
        println!("{}", format!("File: {}", path.display()).dimmed());
    }
}

fn emit_error(format: &str, code: &str, detail: &str) -> ! {
    if is_json(format) {
        eprintln!("{}", json!({"error": code, "detail": detail}));
    } else {
        eprintln!("{}", format!("Error: {}", detail).red());
    }
    process::exit(1);
}

/// Resolve API URL and key from CLI args, env vars, and config (in priority order).
fn resolve_api_credentials(
    api_url: Option<String>,
    api_key: Option<String>,
    config: &lattice::LatticeConfig,
    format: &str,
) -> (String, String) {
    let url = api_url
        .or_else(|| std::env::var("LATTICE_API_URL").ok())
        .or(config.api_url.clone())
        .unwrap_or_else(|| {
            emit_error(
                format,
                "no_api_url",
                "No API URL configured. Set api_url in .lattice/config.yaml, pass --api-url, or set LATTICE_API_URL",
            );
        });

    let key = api_key
        .or_else(|| std::env::var("LATTICE_API_KEY").ok())
        .or(config.api_key.clone())
        .unwrap_or_else(|| {
            emit_error(
                format,
                "no_api_key",
                "No API key configured. Pass --api-key or set LATTICE_API_KEY",
            );
        });

    (url, key)
}

fn install_agent_definitions(root: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
    let agents_dir = root.join(".claude").join("agents");
    std::fs::create_dir_all(&agents_dir).map_err(|e| e.to_string())?;

    let mut created = Vec::new();

    let po_path = agents_dir.join("product-owner.md");
    std::fs::write(&po_path, include_str!("../agents/product-owner.md"))
        .map_err(|e| e.to_string())?;
    created.push(po_path);

    Ok(created)
}

fn install_skill_definitions(root: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
    let skill_dir = root.join(".claude").join("skills").join("lattice");
    std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;

    let skill_path = skill_dir.join("SKILL.md");
    std::fs::write(&skill_path, include_str!("../skills/lattice/SKILL.md"))
        .map_err(|e| e.to_string())?;

    Ok(vec![skill_path])
}

fn detect_claude_code(root: &std::path::Path) -> bool {
    root.join("CLAUDE.md").exists() || root.join(".claude").exists()
}

fn print_created_paths(paths: &[std::path::PathBuf], cwd: &std::path::Path) {
    for path in paths {
        let display = path.strip_prefix(cwd).unwrap_or(path);
        println!("{}", format!("Created {}", display.display()).green());
    }
}

/// Install skill and/or agent definitions, printing results and handling errors.
/// If `fatal` is true, errors cause process::exit(1); otherwise they print a warning.
fn install_extras(cwd: &std::path::Path, include_skill: bool, fatal: bool) {
    if include_skill {
        match install_skill_definitions(cwd) {
            Ok(paths) => print_created_paths(&paths, cwd),
            Err(e) if fatal => {
                eprintln!("{}", format!("Error: {}", e).red());
                process::exit(1);
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    format!("Warning: failed to install skill: {}", e).yellow()
                );
            }
        }
    }
    match install_agent_definitions(cwd) {
        Ok(paths) => print_created_paths(&paths, cwd),
        Err(e) if fatal => {
            eprintln!("{}", format!("Error: {}", e).red());
            process::exit(1);
        }
        Err(e) => {
            eprintln!(
                "{}",
                format!("Warning: failed to install agents: {}", e).yellow()
            );
        }
    }
}

fn prompt_yes_no(message: &str) -> bool {
    use std::io::{self, BufRead, Write};

    eprint!("{} ", message);
    io::stderr().flush().ok();

    let stdin = io::stdin();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

fn print_plan(plan: &Plan, index: &lattice::NodeIndex) {
    // Print summary
    println!("{}", "IMPLEMENTATION PLAN".bold());
    println!(
        "{}",
        format!(
            "{} ready, {} blocked, {} verified\n",
            plan.ready.len(),
            plan.blocked.len(),
            plan.verified.len()
        )
        .dimmed()
    );

    // Print verified (already done)
    if !plan.verified.is_empty() {
        println!("{}", "✓ VERIFIED (already done)".green());
        for id in &plan.verified {
            if let Some(node) = index.get(id) {
                println!("  {} - {}", id.dimmed(), node.title);
            }
        }
        println!();
    }

    // Print ready (can implement now)
    if !plan.ready.is_empty() {
        println!("{}", "→ READY (can implement now)".cyan());
        for id in &plan.ready {
            if let Some(node) = index.get(id) {
                println!("  {} - {}", id.cyan(), node.title);
            }
        }
        println!();
    }

    // Print blocked (waiting on dependencies)
    if !plan.blocked.is_empty() {
        println!("{}", "⏸ BLOCKED (waiting on dependencies)".yellow());
        for item in &plan.items {
            if plan.blocked.contains(&item.id) {
                let status = match &item.resolution {
                    Some(Resolution::Blocked) => "[blocked]".red().to_string(),
                    Some(Resolution::Deferred) => "[deferred]".yellow().to_string(),
                    _ => format!("[needs: {}]", item.blocked_by.join(", "))
                        .dimmed()
                        .to_string(),
                };
                println!("  {} {} - {}", item.id.yellow(), status, item.title);
            }
        }
        println!();
    }

    // Print implementation order
    println!("{}", "IMPLEMENTATION ORDER".bold());
    for item in &plan.items {
        if plan.verified.contains(&item.id) {
            continue; // Skip already verified
        }
        let marker = if plan.ready.contains(&item.id) {
            "→".cyan()
        } else {
            "·".dimmed()
        };
        let status = match &item.resolution {
            Some(Resolution::Verified) => "[verified]".green(),
            Some(Resolution::Blocked) => "[blocked]".red(),
            Some(Resolution::Deferred) => "[deferred]".yellow(),
            Some(Resolution::Wontfix) => "[wontfix]".dimmed(),
            None => "".normal(),
        };
        println!(
            "  {} {}. {} {} - {}",
            marker,
            item.order + 1,
            item.id,
            status,
            item.title
        );
    }
}

fn summarize_edges(edges: &lattice::types::Edges) -> Option<String> {
    let edge_fields: Vec<(&str, usize)> = [
        (
            "supported_by",
            edges.supported_by.as_ref().map_or(0, |v| v.len()),
        ),
        (
            "derives_from",
            edges.derives_from.as_ref().map_or(0, |v| v.len()),
        ),
        (
            "depends_on",
            edges.depends_on.as_ref().map_or(0, |v| v.len()),
        ),
        ("satisfies", edges.satisfies.as_ref().map_or(0, |v| v.len())),
        ("extends", edges.extends.as_ref().map_or(0, |v| v.len())),
        (
            "reveals_gap_in",
            edges.reveals_gap_in.as_ref().map_or(0, |v| v.len()),
        ),
        (
            "challenges",
            edges.challenges.as_ref().map_or(0, |v| v.len()),
        ),
        ("validates", edges.validates.as_ref().map_or(0, |v| v.len())),
        (
            "conflicts_with",
            edges.conflicts_with.as_ref().map_or(0, |v| v.len()),
        ),
        (
            "supersedes",
            edges.supersedes.as_ref().map_or(0, |v| v.len()),
        ),
    ]
    .iter()
    .filter(|(_, count)| *count > 0)
    .cloned()
    .collect();

    if edge_fields.is_empty() {
        None
    } else {
        let parts: Vec<String> = edge_fields
            .iter()
            .map(|(name, count)| format!("{} {}", count, name))
            .collect();
        Some(format!("Edges: {}", parts.join(", ")))
    }
}

fn build_command_catalog() -> serde_json::Value {
    let param = |name: &str, typ: &str, required: bool, desc: &str| -> serde_json::Value {
        json!({
            "name": name,
            "type": typ,
            "required": required,
            "description": desc
        })
    };

    let param_s =
        |name: &str, short: &str, typ: &str, required: bool, desc: &str| -> serde_json::Value {
            json!({
                "name": name,
                "short": short,
                "type": typ,
                "required": required,
                "description": desc
            })
        };

    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Lattice is a knowledge coordination protocol that connects research, strategy, requirements, and implementation into a traversable, version-aware graph. File-based (YAML in .lattice/), Git-native.",
        "concepts": {
            "node_types": {
                "source": {
                    "prefix": "SRC-",
                    "purpose": "Primary research — papers, articles, data, references. The evidence layer.",
                    "connects_to": "Supports theses via 'supported_by' edges"
                },
                "thesis": {
                    "prefix": "THX-",
                    "purpose": "Strategic claims derived from research. Positions your team is taking.",
                    "connects_to": "Derives requirements via 'derives_from' edges. Can be challenged or validated."
                },
                "requirement": {
                    "prefix": "REQ-",
                    "purpose": "Testable specifications. What needs to be built and why.",
                    "connects_to": "Satisfied by implementations. Can depend on other requirements."
                },
                "implementation": {
                    "prefix": "IMP-",
                    "purpose": "Code that satisfies requirements. Binds files to specifications.",
                    "connects_to": "Satisfies requirements. Can reveal gaps or validate theses."
                },
                "message": {
                    "prefix": "MSG-",
                    "purpose": "Persona-specific claims grounded in strategic theses. The outward-facing messaging layer.",
                    "connects_to": "Grounded in theses via 'grounded_in' edges. Drift-detected when theses weaken."
                }
            },
            "edges": {
                "supported_by": "Source supports a thesis — research backing a strategic claim. Direction: thesis → source.",
                "derives_from": "Requirement derives from a thesis — specification grounded in strategy. Direction: requirement → thesis.",
                "depends_on": "Requirement depends on another requirement — must be satisfied first. Direction: requirement → requirement.",
                "satisfies": "Implementation satisfies a requirement — code fulfills a specification. Direction: implementation → requirement.",
                "extends": "Node extends another node — adds capability without replacing. Direction: any → any.",
                "reveals_gap_in": "Implementation discovered a gap in a requirement or thesis — feedback edge, knowledge flowing upstream from code. Direction: implementation → requirement/thesis.",
                "challenges": "Evidence contradicts a thesis — signals the thesis may need revision. Direction: any → thesis.",
                "validates": "Implementation confirms a thesis through working code — positive feedback. Direction: implementation → thesis.",
                "conflicts_with": "Two nodes make incompatible claims — needs resolution. Direction: any → any.",
                "supersedes": "Node replaces an older node — the old node is deprecated. Direction: new → old.",
                "rebuts": "Thesis directly argues against another thesis — structured adversarial debate. Direction: thesis → thesis.",
                "concedes": "Thesis acknowledges a valid point from an opposing thesis — partial agreement in debate. Direction: thesis → thesis.",
                "grounded_in": "Message is grounded in a thesis — messaging claim traces back to strategic position. Direction: message → thesis."
            },
            "versions": "All nodes use semver (MAJOR.MINOR.PATCH). Edges record the target node's version at binding time. When a node is edited, its version bumps and edges bound to the old version become 'potentially stale' — this is drift.",
            "id_conventions": "IDs follow the pattern PREFIX-CATEGORY-NNN (e.g. REQ-CORE-001, THX-AGENT-PROTOCOL, SRC-MCP-SPEC). Categories group related nodes (CORE, CLI, API, AGENT, DIST, etc.)."
        },
        "workflows": [
            {
                "name": "capture_decision",
                "description": "Record a decision from research through to implementation",
                "steps": [
                    "lattice add source --id SRC-... --title '...' --body '...'",
                    "lattice add thesis --id THX-... --title '...' --body '...' --supported-by SRC-...",
                    "lattice add requirement --id REQ-... --title '...' --body '...' --derives-from THX-...",
                    "lattice add implementation --id IMP-... --title '...' --body '...' --satisfies REQ-...",
                    "lattice resolve REQ-... --verified"
                ]
            },
            {
                "name": "check_health",
                "description": "Assess the current state of the lattice for issues and gaps",
                "steps": [
                    "lattice summary",
                    "lattice drift",
                    "lattice lint --strict",
                    "lattice search --priority P0 --resolution unresolved"
                ]
            },
            {
                "name": "respond_to_drift",
                "description": "Investigate and resolve version drift flagged by drift detection",
                "steps": [
                    "lattice drift --format json",
                    "lattice get <flagged-node-id>",
                    "lattice edit <flagged-node-id> --body 'Updated to reflect upstream changes'",
                    "lattice drift --acknowledge <flagged-node-id>"
                ]
            },
            {
                "name": "record_gap",
                "description": "Record a gap discovered during implementation — knowledge flowing upstream from code to requirements",
                "steps": [
                    "lattice refine <parent-req-id> --gap-type <type> --title '...' --description '...' --implementation <imp-id>",
                    "lattice drift"
                ]
            },
            {
                "name": "adversarial_debate",
                "description": "Challenge a thesis with a counter-thesis and track the debate outcome",
                "steps": [
                    "lattice get <thesis-id>",
                    "lattice add thesis --id THX-COUNTER-... --title '...' --body '...' --category technical",
                    "lattice add edge --from THX-COUNTER-... --type rebuts --to <thesis-id>",
                    "lattice edit <thesis-id> --status contested",
                    "lattice assess"
                ]
            },
            {
                "name": "resolve_code_impact",
                "description": "Clear a health FAIL caused by code changes to lattice-tracked files without a corresponding lattice update",
                "steps": [
                    "lattice health --format json",
                    "lattice get <affected-implementation-id>",
                    "lattice verify <IMP-id> satisfies <REQ-id> --tests-pass",
                    "lattice edit <IMP-id> --files 'updated-file-list'",
                    "git add .lattice/",
                    "lattice health --strict --check"
                ]
            }
        ],
        "commands": [
            {
                "name": "init",
                "description": "Initialize a new lattice in the current directory. Run once at project start to create the .lattice/ directory structure.",
                "parameters": [
                    param("--force", "bool", false, "Overwrite existing lattice"),
                    param("--agents", "bool", false, "Install agent definitions (.claude/agents/)"),
                    param("--skill", "bool", false, "Install Claude Code skill and agent definitions (.claude/skills/lattice/)")
                ],
                "examples": [
                    {"command": "lattice init", "explanation": "Create a new .lattice/ directory with default config"},
                    {"command": "lattice init --skill", "explanation": "Initialize and install the Claude Code skill for LLM integration"}
                ],
                "related_commands": ["list", "summary", "add source"]
            },
            {
                "name": "list",
                "description": "List nodes of a given type with optional filters. Use to browse what exists before adding or modifying nodes.",
                "parameters": [
                    param("node_type", "string", true, "Node type: sources, theses, requirements, implementations"),
                    param_s("--status", "-s", "string", false, "Filter by status"),
                    param_s("--priority", "-p", "string", false, "Filter by priority (P0, P1, P2)"),
                    param("--blocked", "bool", false, "Show only blocked items"),
                    param("--deferred", "bool", false, "Show only deferred items"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "[{ id, version, priority?, title, status?, resolution? }]",
                "examples": [
                    {"command": "lattice list requirements", "explanation": "See all requirements with their status and priority"},
                    {"command": "lattice list requirements --priority P0 --format json", "explanation": "Get critical requirements as JSON for programmatic use"},
                    {"command": "lattice list theses", "explanation": "Browse strategic claims to understand project direction"}
                ],
                "related_commands": ["get", "search", "summary"]
            },
            {
                "name": "get",
                "description": "Get a specific node by ID with full details including edges and resolution. Use when you know the node ID and need complete context.",
                "parameters": [
                    param("id", "string", true, "Node ID (e.g. REQ-CORE-001)"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ id, title, body, status, version, priority?, category?, tags[], edges: { derives_from[], depends_on[], satisfies[], ... }, resolution?, created_at, created_by }",
                "examples": [
                    {"command": "lattice get REQ-CORE-001", "explanation": "View a requirement with its edges and resolution status"},
                    {"command": "lattice get THX-AGENT-NATIVE-TOOLS --format json", "explanation": "Get full node data as JSON for processing"}
                ],
                "related_commands": ["list", "search", "edit"]
            },
            {
                "name": "search",
                "description": "Search nodes with filters. More flexible than list — supports text search, tag filtering, and cross-references. Use when you need to find nodes matching complex criteria.",
                "parameters": [
                    param("node_type", "string", false, "Node type (positional or -t): sources, theses, requirements, implementations (default: requirements)"),
                    param_s("--node-type", "-t", "string", false, "Node type to search (alternative to positional arg)"),
                    param_s("--query", "-q", "string", false, "Text search in title and body"),
                    param_s("--priority", "-p", "string", false, "Filter by priority (P0, P1, P2)"),
                    param_s("--resolution", "-r", "string", false, "Filter: verified, blocked, deferred, wontfix, unresolved"),
                    param("--tag", "string", false, "Filter by single tag"),
                    param("--tags", "string", false, "Comma-separated tags (all must match)"),
                    param_s("--category", "-c", "string", false, "Filter by category"),
                    param("--id-prefix", "string", false, "Filter by ID prefix"),
                    param("--related-to", "string", false, "Find nodes related to this node ID"),
                    param("--index", "bool", false, "Build or rebuild the search index"),
                    param("--index-status", "bool", false, "Show search index health"),
                    param("--semantic", "bool", false, "Use vector search (requires vector-search feature + index)"),
                    param("--limit", "integer", false, "Max results to return (default: 20 for semantic)"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "[{ id, version, priority?, title, resolution? }]",
                "examples": [
                    {"command": "lattice search requirements -q 'vibes'", "explanation": "Find requirements mentioning a keyword"},
                    {"command": "lattice search --priority P0 --resolution unresolved", "explanation": "Find critical unresolved requirements — the most urgent work"},
                    {"command": "lattice search --related-to THX-AGENT-NATIVE-TOOLS", "explanation": "Find all nodes connected to a thesis"},
                    {"command": "lattice search --tag agent --category AGENT", "explanation": "Filter by tag and category simultaneously"}
                ],
                "related_commands": ["list", "get"]
            },
            {
                "name": "add requirement",
                "description": "Add a requirement — a testable specification derived from theses. Create when a strategic claim needs to be broken down into buildable work.",
                "parameters": [
                    param("--id", "string", true, "Requirement ID (e.g. REQ-API-003)"),
                    param("--title", "string", true, "Requirement title"),
                    param("--body", "string", true, "Requirement body/description"),
                    param("--priority", "string", true, "Priority: P0, P1, P2"),
                    param("--category", "string", true, "Category (e.g. API, CORE, CLI)"),
                    param("--tags", "string", false, "Comma-separated tags"),
                    param("--derives-from", "string", false, "Comma-separated thesis IDs"),
                    param("--depends-on", "string", false, "Comma-separated requirement IDs"),
                    param("--status", "string", false, "Status: draft, active (default: active)"),
                    param("--created-by", "string", false, "Author (e.g. human:george)"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ id, title, version, status }",
                "examples": [
                    {"command": "lattice add requirement --id REQ-FEAT-001 --title 'New feature' --body 'Description' --priority P1 --category FEAT --derives-from THX-AGENT-PROTOCOL", "explanation": "Add a requirement traced back to a thesis"}
                ],
                "related_commands": ["add thesis", "resolve", "refine", "verify"]
            },
            {
                "name": "add thesis",
                "description": "Add a thesis — a strategic position your team is taking, backed by sources. Create when you have synthesized research into a decision or direction.",
                "parameters": [
                    param("--id", "string", true, "Thesis ID (e.g. THX-AGENT-PROTOCOL)"),
                    param("--title", "string", true, "Thesis title"),
                    param("--body", "string", true, "Thesis body/description"),
                    param("--category", "string", true, "Category: value_prop, market, technical, risk, competitive"),
                    param("--confidence", "float", false, "Confidence level 0.0-1.0"),
                    param("--tags", "string", false, "Comma-separated tags"),
                    param("--supported-by", "string", false, "Comma-separated source IDs"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ id, title, version, status }",
                "examples": [
                    {"command": "lattice add thesis --id THX-NEW --title 'Thesis' --body 'Strategic claim based on research' --category technical --supported-by SRC-PAPER-001", "explanation": "Add a thesis linked to its supporting research"}
                ],
                "related_commands": ["add source", "add requirement"]
            },
            {
                "name": "add source",
                "description": "Add a source — research, paper, or reference material that provides evidence. Create when you have new research to anchor strategic claims.",
                "parameters": [
                    param("--id", "string", true, "Source ID (e.g. SRC-PAPER-001)"),
                    param("--title", "string", true, "Source title"),
                    param("--body", "string", true, "Source body/summary"),
                    param("--url", "string", false, "Source URL"),
                    param("--source-type", "string", false, "Type: paper, article, report, data"),
                    param("--tags", "string", false, "Comma-separated tags"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ id, title, version, status }",
                "examples": [
                    {"command": "lattice add source --id SRC-NEW --title 'Research paper' --body 'Key findings summary' --url https://example.com", "explanation": "Add a source with a URL for reference"}
                ],
                "related_commands": ["add thesis", "add edge"]
            },
            {
                "name": "add implementation",
                "description": "Add an implementation — code that satisfies requirements. Create after writing code to record which requirements it addresses and which files are involved.",
                "parameters": [
                    param("--id", "string", true, "Implementation ID (e.g. IMP-STORAGE-001)"),
                    param("--title", "string", true, "Implementation title"),
                    param("--body", "string", true, "Implementation body/description"),
                    param("--satisfies", "string", false, "Comma-separated requirement IDs"),
                    param("--files", "string", false, "Comma-separated file paths"),
                    param("--tags", "string", false, "Comma-separated tags"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ id, title, version, status }",
                "examples": [
                    {"command": "lattice add implementation --id IMP-NEW --title 'Storage layer' --body 'File-based YAML storage' --satisfies REQ-CORE-001 --files 'src/storage.rs'", "explanation": "Record an implementation with its requirement bindings and source files"}
                ],
                "related_commands": ["verify", "resolve", "refine", "add edge"]
            },
            {
                "name": "add message",
                "description": "Add a message — a persona-specific claim grounded in strategic theses. Create when you need to capture what to say to a specific audience and trace it back to evidence.",
                "parameters": [
                    param("--id", "string", true, "Message ID (e.g. MSG-DEV-PERF-001)"),
                    param("--title", "string", true, "Message title"),
                    param("--body", "string", true, "The actual claim or talking point"),
                    param("--persona", "string", true, "Target persona (e.g. developer, investor, operator)"),
                    param("--channel", "string", false, "Comma-separated channels (e.g. docs, landing-page, pitch-deck)"),
                    param("--grounded-in", "string", false, "Comma-separated thesis IDs this message is grounded in"),
                    param("--tags", "string", false, "Comma-separated tags"),
                    param("--created-by", "string", false, "Author (e.g. human:george)"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ id, title, version, status }",
                "examples": [
                    {"command": "lattice add message --id MSG-DEV-001 --title 'Zero infrastructure' --body 'File-based YAML, no servers needed' --persona developer --grounded-in THX-FILE-OVER-DB", "explanation": "Add a developer-facing message grounded in a strategic thesis"}
                ],
                "related_commands": ["add thesis", "list", "drift"]
            },
            {
                "name": "add edge",
                "description": "Add a relationship between two existing nodes. Use for feedback edges (reveals_gap_in, challenges, validates), traceability, or dependencies not captured during node creation.",
                "parameters": [
                    param("--from", "string", true, "Source node ID (edge goes FROM this node)"),
                    param("--edge-type / --type", "string", true, "Edge type: supported_by, derives_from, depends_on, satisfies, extends, reveals_gap_in, challenges, validates, conflicts_with, supersedes"),
                    param("--to", "string", true, "Target node ID (edge goes TO this node)"),
                    param("--rationale", "string", false, "Why this edge exists"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ from, edge_type, to, rationale?, version }",
                "examples": [
                    {"command": "lattice add edge --from IMP-CLI-001 --type reveals_gap_in --to REQ-CORE-005 --rationale 'Requirement does not specify timeout behavior'", "explanation": "During implementation, you discovered the requirement is underspecified. This feedback edge flags the gap."},
                    {"command": "lattice add edge --from IMP-CLI-001 --type validates --to THX-AGENT-NATIVE-TOOLS --rationale 'CLI confirms structured knowledge is queryable'", "explanation": "Working code confirmed a strategic thesis — positive feedback from implementation to strategy."},
                    {"command": "lattice add edge --from IMP-CLI-001 --type challenges --to THX-PROSE-PRIMARY --rationale 'Agents prefer JSON over prose'", "explanation": "Implementation evidence contradicts a thesis — signals the thesis may need revision."}
                ],
                "related_commands": ["remove edge", "replace edge", "refine"]
            },
            {
                "name": "remove edge",
                "description": "Remove an edge between two nodes. Use when a relationship is no longer valid.",
                "parameters": [
                    param("--from", "string", true, "Source node ID (edge originates FROM this node)"),
                    param("--edge-type / --type", "string", true, "Edge type to remove"),
                    param("--to", "string", true, "Target node ID"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    {"command": "lattice remove edge --from IMP-CLI-001 --type satisfies --to REQ-CORE-001", "explanation": "Remove a satisfaction binding that is no longer accurate"}
                ],
                "related_commands": ["add edge", "replace edge"]
            },
            {
                "name": "replace edge",
                "description": "Retarget an existing edge to a new node. Use when a requirement is split or reorganized and existing edges need to follow.",
                "parameters": [
                    param("--from", "string", true, "Source node ID"),
                    param("--edge-type / --type", "string", true, "Edge type"),
                    param("--old-to", "string", true, "Current target node ID to replace"),
                    param("--new-to", "string", true, "New target node ID"),
                    param("--rationale", "string", false, "Updated rationale for the edge"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    {"command": "lattice replace edge --from IMP-CLI-001 --type satisfies --old-to REQ-CORE-001 --new-to REQ-CORE-002 --rationale 'Requirement was split'", "explanation": "Retarget an implementation's satisfaction edge after a requirement was reorganized"}
                ],
                "related_commands": ["add edge", "remove edge"]
            },
            {
                "name": "resolve",
                "description": "Set the resolution status of a requirement. Use after verifying an implementation satisfies it, or to mark it blocked/deferred/wontfix.",
                "parameters": [
                    param("id", "string", true, "Requirement ID"),
                    param("--verified", "bool", false, "Mark as verified"),
                    param("--blocked", "string", false, "Mark as blocked with reason"),
                    param("--deferred", "string", false, "Mark as deferred with reason"),
                    param("--wontfix", "string", false, "Mark as wontfix with reason"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ id, resolution: { status, reason?, resolved_at } }",
                "examples": [
                    {"command": "lattice resolve REQ-CORE-001 --verified", "explanation": "Mark a requirement as verified after confirming the implementation works"},
                    {"command": "lattice resolve REQ-API-002 --deferred 'Post-MVP'", "explanation": "Defer a requirement with a reason — it stays tracked but is not blocking"}
                ],
                "related_commands": ["verify", "get", "list"]
            },
            {
                "name": "edit",
                "description": "Edit fields on an existing node. Auto-bumps the patch version, which may trigger drift on downstream edges. Use when a node's content needs updating.",
                "parameters": [
                    param("id", "string", true, "Node ID (e.g. REQ-CORE-001)"),
                    param("--title", "string", false, "New title"),
                    param("--body", "string", false, "New body"),
                    param("--status", "string", false, "New status: draft, active, deprecated, superseded"),
                    param("--priority", "string", false, "New priority: P0, P1, P2 (requirements only)"),
                    param("--tags", "string", false, "Comma-separated tags (replaces existing)"),
                    param("--category", "string", false, "New category"),
                    param("--files", "string", false, "Comma-separated file paths (replaces existing, implementations only)"),
                    param("--test-command", "string", false, "Test command (implementations only)"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ id, title, version, changed_fields[] }",
                "examples": [
                    {"command": "lattice edit REQ-CORE-001 --title 'Updated title' --priority P0", "explanation": "Update a requirement's title and escalate its priority"},
                    {"command": "lattice edit IMP-CLI-001 --files 'src/main.rs,src/lib.rs' --test-command 'cargo test'", "explanation": "Update an implementation's file bindings and test command"}
                ],
                "related_commands": ["get", "drift"]
            },
            {
                "name": "verify",
                "description": "Record that an implementation satisfies a requirement with evidence. Use after running tests to formally bind implementation to requirement with proof.",
                "parameters": [
                    param("implementation", "string", true, "Implementation ID"),
                    param("relation", "string", true, "Must be 'satisfies'"),
                    param("requirement", "string", true, "Requirement ID"),
                    param("--tests-pass", "bool", false, "Record that tests pass"),
                    param("--coverage", "float", false, "Coverage percentage 0.0-1.0"),
                    param("--files", "string", false, "Comma-separated evidence file paths"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    {"command": "lattice verify IMP-STORAGE-001 satisfies REQ-CORE-004 --tests-pass --coverage 0.94", "explanation": "Record that tests pass with 94% coverage as evidence of satisfaction"}
                ],
                "related_commands": ["resolve", "add implementation"]
            },
            {
                "name": "refine",
                "description": "Create a sub-requirement when implementation reveals a requirement is underspecified. This is a feedback edge — knowledge flowing upstream from code to requirements.",
                "parameters": [
                    param("parent", "string", true, "Parent requirement ID"),
                    param("--gap-type", "string", true, "Gap type: clarification, design_decision, missing_requirement, contradiction"),
                    param("--title", "string", true, "Brief title for the sub-requirement"),
                    param("--description", "string", true, "What is underspecified and why"),
                    param("--proposed / --proposal", "string", false, "Proposed resolution (alias: --proposal)"),
                    param("--implementation / --discovered-by", "string", false, "Implementation ID that discovered this gap (alias: --discovered-by)"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ id, parent, gap_type, title }",
                "examples": [
                    {"command": "lattice refine REQ-CORE-005 --gap-type design_decision --title 'Drift threshold' --description 'Should minor version drift be flagged?' --implementation IMP-GRAPH-001", "explanation": "During implementation, discovered the requirement doesn't specify drift sensitivity — create a sub-requirement to resolve the ambiguity"}
                ],
                "related_commands": ["add edge", "drift", "resolve"]
            },
            {
                "name": "drift",
                "description": "Check whether upstream knowledge has changed since downstream nodes were last reviewed. Run after editing sources or theses to see which requirements need attention.",
                "parameters": [
                    param("--check", "bool", false, "Exit with code 2 if drift detected"),
                    param("--acknowledge", "string", false, "Node ID to acknowledge drift on (re-snapshots edge versions)"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ drift_detected: bool, items: [{ node_id, edge_type, target_id, bound_version, current_version }] }",
                "examples": [
                    {"command": "lattice drift", "explanation": "Check all edges for version drift — shows which nodes have stale bindings"},
                    {"command": "lattice drift --check --format json", "explanation": "Machine-readable drift check — exits non-zero if drift exists (useful in CI)"},
                    {"command": "lattice drift --acknowledge REQ-INFRA-015", "explanation": "After reviewing a node, re-snapshot its edge versions to clear the drift warning"}
                ],
                "related_commands": ["get", "edit", "summary"]
            },
            {
                "name": "lint",
                "description": "Check lattice files for structural issues like missing fields, broken edges, or invalid references. Run as part of health checks or CI.",
                "parameters": [
                    param("--fix", "bool", false, "Attempt to auto-fix fixable issues"),
                    param("--strict", "bool", false, "Exit with non-zero status on any issue"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ issues: [{ severity, node_id?, message, fixable }], total, fixable_count }",
                "examples": [
                    {"command": "lattice lint", "explanation": "Check for structural issues across all lattice files"},
                    {"command": "lattice lint --fix", "explanation": "Auto-fix issues like missing config fields or malformed references"},
                    {"command": "lattice lint --strict --format json", "explanation": "Strict mode for CI — exits non-zero on any issue"}
                ],
                "related_commands": ["drift", "summary", "freshness"]
            },
            {
                "name": "freshness",
                "description": "Check if lattice has been updated alongside code changes. Compares the last git commit touching .lattice/ vs code files. Use in pre-commit hooks or CI to catch stale lattice.",
                "parameters": [
                    param("--threshold", "integer", false, "Maximum allowed gap in hours between code and lattice updates (default: 72)"),
                    param("--check", "bool", false, "Exit with code 2 if lattice is stale (for CI/hooks)"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ stale: bool, lattice_last_updated, code_last_updated, gap_hours, threshold_hours }",
                "examples": [
                    {"command": "lattice freshness", "explanation": "Check if lattice is up to date relative to code changes"},
                    {"command": "lattice freshness --check", "explanation": "Exit non-zero if lattice is stale — use in CI or pre-commit hooks"},
                    {"command": "lattice freshness --threshold 24 --check", "explanation": "Stricter threshold — flag if lattice hasn't been updated within 24h of code changes"},
                    {"command": "lattice freshness --format json", "explanation": "Machine-readable freshness check for dashboards"}
                ],
                "related_commands": ["drift", "lint", "summary", "assess"]
            },
            {
                "name": "assess",
                "description": "Assess change pressure — how many theses are contested, how many requirements are affected, and whether the graph needs a planning cycle.",
                "parameters": [
                    param("--check", "bool", false, "Exit with code 2 if change pressure is non-zero (for CI/hooks)"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ contested_theses, theses_with_confidence_changes, affected_requirements, drift_items, messages, change_pressure }",
                "examples": [
                    {"command": "lattice assess", "explanation": "Show change pressure metrics — contested theses, affected requirements, drift"},
                    {"command": "lattice assess --check", "explanation": "Exit non-zero if any change pressure exists — use in CI to trigger planning cycles"},
                    {"command": "lattice assess --format json", "explanation": "Machine-readable assessment for automation"}
                ],
                "related_commands": ["drift", "freshness", "health", "summary", "plan"]
            },
            {
                "name": "health",
                "description": "Unified health check combining freshness, change pressure, and code impact into a single PASS/WARN/FAIL verdict. Use as the single CI gate for lattice health.",
                "parameters": [
                    param("--check", "bool", false, "Exit with code 2 on FAIL verdict (for CI/hooks)"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ verdict, freshness: { gap_hours }, change_pressure: { contested_theses, drift_items, total }, code_impact: { total_files_changed, tracked_files_changed, bound_files_count } }",
                "examples": [
                    {"command": "lattice health", "explanation": "Check overall lattice health — combines freshness, change pressure, and code impact"},
                    {"command": "lattice health --check", "explanation": "CI gate — exits non-zero if lattice health is FAIL"},
                    {"command": "lattice health --format json", "explanation": "Machine-readable health report for automation"}
                ],
                "related_commands": ["freshness", "assess", "drift", "summary"]
            },
            {
                "name": "summary",
                "description": "Show a compact status overview — node counts, resolution breakdown, drift status, and orphaned nodes. Start here to understand the lattice's current state.",
                "parameters": [
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ nodes: { sources, theses, requirements, implementations }, resolutions: { verified, blocked, deferred, unresolved }, drift: { detected, count }, orphans[] }",
                "examples": [
                    {"command": "lattice summary", "explanation": "Quick overview of lattice health — how many nodes, what's resolved, any drift"},
                    {"command": "lattice summary --format json", "explanation": "Machine-readable summary for dashboards or agent consumption"}
                ],
                "related_commands": ["drift", "lint", "list"]
            },
            {
                "name": "diff",
                "description": "Show lattice nodes added, modified, or resolved since a git ref. Use to understand what changed on a branch or since a specific commit.",
                "parameters": [
                    param("--since", "string", false, "Git ref to compare against (default: merge-base with main)"),
                    param("--md", "bool", false, "Output as markdown for GitHub comments"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "output_schema_hint": "{ base_ref, has_changes, total_changes, added[], modified[], resolved[], deleted[] }",
                "examples": [
                    {"command": "lattice diff", "explanation": "Show lattice changes on the current branch vs main"},
                    {"command": "lattice diff --since HEAD~3 --md", "explanation": "Generate markdown summary of changes over the last 3 commits"},
                    {"command": "lattice diff --format json", "explanation": "Machine-readable diff for CI or automated comments"}
                ],
                "related_commands": ["summary", "get"]
            },
            {
                "name": "plan",
                "description": "Plan implementation order for requirements based on their dependency graph. Shows which requirements are ready, which are blocked, and the optimal sequence.",
                "parameters": [
                    param("requirements", "string[]", true, "Requirement IDs to plan")
                ],
                "output_schema_hint": "{ ready[], blocked[], sequence[] }",
                "examples": [
                    {"command": "lattice plan REQ-CLI-001 REQ-CLI-002", "explanation": "Determine the implementation order for two requirements based on dependencies"}
                ],
                "related_commands": ["list", "get", "summary"]
            },
            {
                "name": "export",
                "description": "Export the lattice as a narrative document, JSON data, HTML page, or GitHub Pages site. Use to share lattice state with stakeholders or publish documentation.",
                "parameters": [
                    param("--format", "string", false, "Export format: narrative, json, html, pages (default: narrative)"),
                    param("--audience", "string", false, "Target audience: investor, contributor, overview (default: overview)"),
                    param("--title", "string", false, "Document title (default: Lattice)"),
                    param("--include-internal", "bool", false, "Include nodes marked as internal"),
                    param("--output", "string", false, "Output directory for HTML/pages export")
                ],
                "examples": [
                    {"command": "lattice export", "explanation": "Generate a narrative overview of the lattice for general audiences"},
                    {"command": "lattice export --format json", "explanation": "Export full lattice data as JSON"},
                    {"command": "lattice export --format pages --output _site", "explanation": "Generate a GitHub Pages site with interactive lattice viewer"}
                ],
                "related_commands": ["summary", "list"]
            },
            {
                "name": "update",
                "description": "Self-update lattice to the latest version. Use --check to see if an update is available without installing.",
                "parameters": [
                    param("--check", "bool", false, "Only check for updates, don't install"),
                    param("--force", "bool", false, "Force update even if already up to date"),
                    param("--version", "string", false, "Install a specific version (e.g. 0.1.5)"),
                    param_s("--format", "-f", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    {"command": "lattice update", "explanation": "Update to the latest release"},
                    {"command": "lattice update --check", "explanation": "Check if a newer version is available without installing"},
                    {"command": "lattice update --version 0.1.5", "explanation": "Install a specific version"}
                ],
                "related_commands": []
            },
            {
                "name": "help",
                "description": "Show available commands, or a specific topic (concepts, workflows). Use --json for the full machine-readable catalog (this output).",
                "parameters": [
                    param("topic", "string", false, "Help topic: concepts, workflows (omit for command list)"),
                    param("--json", "bool", false, "Output as structured JSON for agent consumption"),
                    param("--compact", "bool", false, "With --json: output only command signatures (name + parameters), no examples/descriptions/concepts")
                ],
                "examples": [
                    {"command": "lattice help", "explanation": "Show human-readable command list"},
                    {"command": "lattice help concepts", "explanation": "Learn about node types, edge semantics, versioning, and ID conventions"},
                    {"command": "lattice help workflows", "explanation": "See common task-oriented command sequences"},
                    {"command": "lattice help --json", "explanation": "Output the full command catalog as JSON for LLM consumption"},
                    {"command": "lattice help --json --compact", "explanation": "Output compact schema — just command names and parameter specs (~50% smaller)"}
                ],
                "related_commands": []
            }
        ],
        "exit_codes": {
            "0": "Success",
            "1": "Error",
            "2": "Actionable condition (drift detected, lint issues)"
        },
        "global_flags": {
            "--format json": "Available on all read and write commands for structured output",
            "--json": "Output machine-readable command catalog (top-level flag)",
            "--json --compact": "Output compact command catalog — signatures only, no examples/descriptions",
            "--help": "Show help for any command",
            "--version": "Show version"
        }
    })
}

fn command_to_name(cmd: &Commands) -> &'static str {
    match cmd {
        Commands::Init { .. } => "init",
        Commands::Add { .. } => "add",
        Commands::Remove { .. } => "remove",
        Commands::Replace { .. } => "replace",
        Commands::List { .. } => "list",
        Commands::Resolve { .. } => "resolve",
        Commands::Edit { .. } => "edit",
        Commands::Plan { .. } => "plan",
        Commands::Drift { .. } => "drift",
        Commands::Get { .. } => "get",
        Commands::Export { .. } => "export",
        Commands::Summary { .. } => "summary",
        Commands::Lint { .. } => "lint",
        Commands::Freshness { .. } => "freshness",
        Commands::Assess { .. } => "assess",
        Commands::Health { .. } => "health",
        Commands::Verify { .. } => "verify",
        Commands::Refine { .. } => "refine",
        Commands::Search { .. } => "search",
        Commands::Mcp => "mcp",
        Commands::Update { .. } => "update",
        Commands::Prompt { .. } => "prompt",
        Commands::Push { .. } => "push",
        Commands::Migrate => "migrate",
        Commands::Diff { .. } => "diff",
        Commands::Help { .. } => "help",
    }
}

/// Strip a full catalog down to just command signatures (name + parameters).
fn compact_catalog(catalog: &serde_json::Value) -> serde_json::Value {
    let commands = catalog["commands"]
        .as_array()
        .map(|cmds| {
            cmds.iter()
                .map(|cmd| {
                    json!({
                        "name": cmd["name"],
                        "parameters": cmd["parameters"],
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({
        "version": catalog["version"],
        "commands": commands,
    })
}

fn print_grouped_help() {
    let catalog = build_command_catalog();
    println!(
        "{}",
        "Lattice manages sources → theses → requirements → implementations".bold()
    );
    println!(
        "{}\n",
        "Connected by version-tracked edges. Run 'lattice help concepts' for details.".dimmed()
    );

    let groups: &[(&str, &[&str])] = &[
        (
            "KNOWLEDGE GRAPH:",
            &[
                "add source",
                "add thesis",
                "add requirement",
                "add implementation",
                "add edge",
                "add message",
                "get",
                "list",
                "search",
                "edit",
                "resolve",
                "verify",
                "refine",
                "remove edge",
                "replace edge",
            ],
        ),
        ("ANALYSIS:", &["summary", "diff", "plan", "export"]),
        (
            "AUTOMATED CHECKS:",
            &["health", "drift", "freshness", "assess", "lint"],
        ),
        ("SETUP:", &["init", "update", "help"]),
    ];

    if let Some(commands) = catalog["commands"].as_array() {
        for (heading, names) in groups {
            println!("{}", heading.bold());
            for name in *names {
                if let Some(cmd) = commands.iter().find(|c| c["name"].as_str() == Some(name)) {
                    let desc = cmd["description"].as_str().unwrap_or("");
                    let short = desc.find(". ").map(|i| &desc[..i + 1]).unwrap_or(desc);
                    println!("  {:<22} {}", name.cyan(), short);
                }
            }
            println!();
        }
    }

    println!("{}", "OPTIONS:".bold());
    println!(
        "  {:<22} Output machine-readable command catalog as JSON",
        "--json".cyan()
    );
    println!(
        "  {:<22} With --json: compact schema only (signatures, no examples)",
        "--json --compact".cyan()
    );
    println!("  {:<22} Print help", "-h, --help".cyan());
    println!("  {:<22} Print version", "-V, --version".cyan());
    println!();

    println!("{}", "TOPICS:".bold());
    println!(
        "  {:<22} Node types, edge semantics, versioning, ID conventions",
        "lattice help concepts".cyan()
    );
    println!(
        "  {:<22} Common task-oriented command sequences",
        "lattice help workflows".cyan()
    );
    println!(
        "  {:<22} Health dimensions, clearing a FAIL, configuration",
        "lattice help health".cyan()
    );
    println!();
    println!(
        "Use {} for help on a specific command.",
        "lattice <command> --help".cyan()
    );
}

fn main() {
    // Intercept top-level --help/-h and --version/-V before clap parses,
    // so subcommand --help still uses clap's built-in per-command help,
    // and --version can show the passive update notification.
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && (args[1] == "--help" || args[1] == "-h") {
        print_grouped_help();
        return;
    }
    if args.len() == 2 && (args[1] == "--version" || args[1] == "-V") {
        println!("lattice {}", env!("CARGO_PKG_VERSION"));
        lattice::update::maybe_notify_update(None);
        return;
    }

    let cli = Cli::parse();

    // Handle top-level --json flag (outputs command catalog)
    if cli.json {
        let catalog = build_command_catalog();
        let output = if cli.compact {
            compact_catalog(&catalog)
        } else {
            catalog
        };
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        return;
    }

    // If no subcommand given, show grouped help
    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            print_grouped_help();
            lattice::update::maybe_notify_update(None);
            return;
        }
    };

    let command_name = command_to_name(&command);

    // Check schema version (skip for init, migrate, help, update, mcp, prompt)
    let skip_schema_check = matches!(
        command_name,
        "init" | "migrate" | "help" | "update" | "mcp" | "prompt"
    );
    if !skip_schema_check {
        let cwd = std::env::current_dir().unwrap_or_default();
        if let Some(root) = find_lattice_root(&cwd) {
            match check_schema_version(&root) {
                SchemaCheck::Current => {}
                SchemaCheck::NeedsMigration(old_version) => {
                    let label = if old_version.is_empty() {
                        "unknown".to_string()
                    } else {
                        old_version
                    };
                    eprintln!(
                        "{}",
                        format!(
                            "Warning: This lattice uses schema {} (current: {}). Run 'lattice migrate' to update.",
                            label, CURRENT_SCHEMA_VERSION
                        )
                        .yellow()
                    );
                }
                SchemaCheck::BinaryTooOld(repo_version) => {
                    eprintln!(
                        "{}",
                        format!(
                            "Warning: This lattice uses schema {} but your binary supports {}. Run 'lattice update' to get the latest version.",
                            repo_version, CURRENT_SCHEMA_VERSION
                        )
                        .yellow()
                    );
                }
            }
        }
    }

    run_command(command);

    // Passive update check after command output is complete
    lattice::update::maybe_notify_update(Some(command_name));
}

fn run_command(command: Commands) {
    match command {
        Commands::Init {
            force,
            agents,
            skill,
        } => {
            let cwd = env::current_dir().expect("Failed to get current directory");
            let lattice_exists = cwd.join(".lattice").exists();
            let install_agents = agents || skill;

            // If --skill or --agents on an existing lattice, install standalone
            if install_agents && lattice_exists && !force {
                install_extras(&cwd, skill, true);
                println!();
                if skill {
                    println!(
                        "{}",
                        "Skill and agent definitions installed.".green().bold()
                    );
                } else {
                    println!("{}", "Agent definitions installed.".green().bold());
                }
            } else {
                match init_lattice(&cwd, force) {
                    Ok(created) => {
                        print_created_paths(&created, &cwd);

                        if install_agents {
                            install_extras(&cwd, skill, false);
                        } else if detect_claude_code(&cwd)
                            && prompt_yes_no(
                                "Detected CLAUDE.md. Install Lattice skill for Claude Code? (y/N)",
                            )
                        {
                            install_extras(&cwd, true, false);
                        }

                        println!();
                        println!("{}", "Lattice initialized.".green().bold());
                        println!();
                        println!("Next steps:");
                        println!("  lattice seed              # Bootstrap from vision");
                        println!("  lattice add requirement   # Add a requirement manually");
                        if !install_agents {
                            println!(
                                "  lattice init --skill      # Install Claude Code skill + agents"
                            );
                        }
                    }
                    Err(e) => {
                        let err_str = e.to_string();
                        if err_str.contains("already initialized") {
                            println!(
                                "{}",
                                "Lattice already initialized. Use --force to reinitialize."
                                    .dimmed()
                            );
                        } else {
                            eprintln!("{}", format!("Error: {}", e).red());
                            process::exit(1);
                        }
                    }
                }
            }
        }

        Commands::Add { add_command } => match add_command {
            AddCommands::Requirement {
                id,
                title,
                body,
                priority,
                category,
                tags,
                derives_from,
                depends_on,
                status,
                created_by,
                format,
            } => {
                let root = get_lattice_root();
                let priority = parse_priority(&priority);
                let status = parse_status(&status);
                let created_by = created_by.unwrap_or_else(|| {
                    format!("agent:claude-{}", chrono::Utc::now().format("%Y-%m-%d"))
                });

                let options = AddRequirementOptions {
                    id: id.clone(),
                    title,
                    body,
                    priority,
                    category,
                    tags: split_csv(tags),
                    derives_from: split_csv(derives_from),
                    depends_on: split_csv(depends_on),
                    status,
                    created_by,
                };

                match add_requirement(&root, options) {
                    Ok(path) => emit_created(&format, "requirement", &id, &path),
                    Err(e) => emit_error(&format, "add_error", &e.to_string()),
                }
            }

            AddCommands::Thesis {
                id,
                title,
                body,
                category,
                confidence,
                supported_by,
                status,
                created_by,
                format,
            } => {
                let root = get_lattice_root();
                let category = parse_thesis_category(&category);
                let status = parse_status(&status);
                let created_by = created_by.unwrap_or_else(|| "unknown".to_string());

                let options = AddThesisOptions {
                    id: id.clone(),
                    title,
                    body,
                    category,
                    confidence: Some(confidence),
                    supported_by: split_csv(supported_by),
                    status,
                    created_by,
                };

                match add_thesis(&root, options) {
                    Ok(path) => emit_created(&format, "thesis", &id, &path),
                    Err(e) => emit_error(&format, "add_error", &e.to_string()),
                }
            }

            AddCommands::Source {
                id,
                title,
                body,
                url,
                citations,
                reliability,
                status,
                created_by,
                format,
            } => {
                let root = get_lattice_root();
                let reliability = parse_reliability(&reliability);
                let status = parse_status(&status);
                let created_by = created_by.unwrap_or_else(|| "unknown".to_string());

                let options = AddSourceOptions {
                    id: id.clone(),
                    title,
                    body,
                    url,
                    citations: split_csv(citations),
                    reliability,
                    status,
                    created_by,
                };

                match add_source(&root, options) {
                    Ok(path) => emit_created(&format, "source", &id, &path),
                    Err(e) => emit_error(&format, "add_error", &e.to_string()),
                }
            }

            AddCommands::Implementation {
                id,
                title,
                body,
                language,
                files,
                test_command,
                satisfies,
                status,
                created_by,
                format,
            } => {
                let root = get_lattice_root();
                let status = parse_status(&status);
                let created_by = created_by.unwrap_or_else(|| {
                    format!("agent:claude-{}", chrono::Utc::now().format("%Y-%m-%d"))
                });

                let options = AddImplementationOptions {
                    id: id.clone(),
                    title,
                    body,
                    language,
                    files: split_csv(files),
                    test_command,
                    satisfies: split_csv(satisfies),
                    status,
                    created_by,
                };

                match add_implementation(&root, options) {
                    Ok(path) => emit_created(&format, "implementation", &id, &path),
                    Err(e) => emit_error(&format, "add_error", &e.to_string()),
                }
            }

            AddCommands::Edge {
                from,
                edge_type,
                to,
                rationale,
                format,
            } => {
                let root = get_lattice_root();

                let options = AddEdgeOptions {
                    from_id: from.clone(),
                    edge_type: edge_type.clone(),
                    to_id: to.clone(),
                    rationale,
                };

                match add_edge(&root, options) {
                    Ok(path) => {
                        if is_json(&format) {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&json!({
                                    "success": true,
                                    "from": from,
                                    "edge_type": edge_type,
                                    "to": to,
                                    "file": path.display().to_string(),
                                }))
                                .unwrap()
                            );
                        } else {
                            println!(
                                "{}",
                                format!("Added edge: {} --[{}]--> {}", from, edge_type, to).green()
                            );
                            println!("{}", format!("File: {}", path.display()).dimmed());
                        }
                    }
                    Err(e) => emit_error(&format, "add_edge_error", &e.to_string()),
                }
            }
            AddCommands::Message {
                id,
                title,
                body,
                persona,
                channel,
                grounded_in,
                tags,
                created_by,
                format,
            } => {
                let root = get_lattice_root();

                let options = AddMessageOptions {
                    id: id.clone(),
                    title,
                    body,
                    persona,
                    channel: split_csv(channel),
                    grounded_in: split_csv(grounded_in),
                    tags: split_csv(tags),
                    status: Status::Active,
                    created_by: created_by
                        .unwrap_or_else(|| get_git_user().unwrap_or_else(|| "unknown".to_string())),
                };

                match add_message(&root, options) {
                    Ok(path) => emit_created(&format, "message", &id, &path),
                    Err(e) => emit_error(&format, "add_message_error", &e.to_string()),
                }
            }
        },

        Commands::Remove { remove_command } => match remove_command {
            RemoveCommands::Edge {
                from,
                edge_type,
                to,
                format,
            } => {
                let root = get_lattice_root();

                let options = RemoveEdgeOptions {
                    from_id: from.clone(),
                    edge_type: edge_type.clone(),
                    to_id: to.clone(),
                };

                match remove_edge(&root, options) {
                    Ok(path) => {
                        if is_json(&format) {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&json!({
                                    "success": true,
                                    "from": from,
                                    "edge_type": edge_type,
                                    "to": to,
                                    "file": path.display().to_string(),
                                }))
                                .unwrap()
                            );
                        } else {
                            println!(
                                "{}",
                                format!("Removed edge: {} --[{}]--> {}", from, edge_type, to)
                                    .green()
                            );
                            println!("{}", format!("File: {}", path.display()).dimmed());
                        }
                    }
                    Err(e) => emit_error(&format, "remove_edge_error", &e.to_string()),
                }
            }
        },

        Commands::Replace { replace_command } => match replace_command {
            ReplaceCommands::Edge {
                from,
                edge_type,
                old_to,
                new_to,
                rationale,
                format,
            } => {
                let root = get_lattice_root();

                let options = ReplaceEdgeOptions {
                    from_id: from.clone(),
                    edge_type: edge_type.clone(),
                    old_to_id: old_to.clone(),
                    new_to_id: new_to.clone(),
                    rationale,
                };

                match replace_edge(&root, options) {
                    Ok(path) => {
                        if is_json(&format) {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&json!({
                                    "success": true,
                                    "from": from,
                                    "edge_type": edge_type,
                                    "old_to": old_to,
                                    "new_to": new_to,
                                    "file": path.display().to_string(),
                                }))
                                .unwrap()
                            );
                        } else {
                            println!(
                                "{}",
                                format!(
                                    "Replaced edge: {} --[{}]--> {} (was {})",
                                    from, edge_type, new_to, old_to
                                )
                                .green()
                            );
                            println!("{}", format!("File: {}", path.display()).dimmed());
                        }
                    }
                    Err(e) => emit_error(&format, "replace_edge_error", &e.to_string()),
                }
            }
        },

        Commands::List {
            node_type,
            status,
            priority,
            blocked,
            deferred,
            format,
        } => {
            let root = get_lattice_root();

            // Validate --status if provided
            let status_filter = status.as_ref().map(|s| {
                s.parse::<Status>().unwrap_or_else(|e| {
                    emit_error(&format, "invalid_status", &e);
                })
            });

            // Validate --priority if provided
            let priority_filter = priority.as_ref().map(|p| {
                p.parse::<Priority>().unwrap_or_else(|e| {
                    emit_error(&format, "invalid_priority", &e);
                })
            });

            let type_name = match node_type.as_str() {
                "sources" | "theses" | "requirements" | "implementations" | "messages" => {
                    node_type.as_str()
                }
                _ => emit_error(
                    &format,
                    "unknown_type",
                    &format!("Unknown type: {}", node_type),
                ),
            };

            match load_nodes_by_type(&root, type_name) {
                Ok(nodes) => {
                    // Apply filters
                    let filtered: Vec<_> = nodes
                        .into_iter()
                        .filter(|n| {
                            // --status filter (workflow status: draft, active, deprecated, superseded)
                            if let Some(ref sf) = status_filter
                                && &n.status != sf
                            {
                                return false;
                            }
                            // --priority filter
                            if let Some(ref pf) = priority_filter
                                && n.priority.as_ref() != Some(pf)
                            {
                                return false;
                            }
                            // --blocked / --deferred flags (resolution filters)
                            if blocked || deferred {
                                return n.resolution.as_ref().is_some_and(|r| {
                                    (blocked && matches!(r.status, Resolution::Blocked))
                                        || (deferred && matches!(r.status, Resolution::Deferred))
                                });
                            }
                            true
                        })
                        .collect();

                    if is_json(&format) {
                        let json_nodes: Vec<_> = filtered
                            .iter()
                            .map(|n| {
                                json!({
                                    "id": n.id,
                                    "title": n.title,
                                    "type": format!("{:?}", n.node_type).to_lowercase(),
                                    "status": format!("{:?}", n.status).to_lowercase(),
                                    "version": n.version,
                                    "priority": n.priority.as_ref().map(|p| format!("{:?}", p)),
                                    "category": n.category,
                                    "resolution": n.resolution.as_ref().map(|r| format!("{:?}", r.status).to_lowercase()),
                                    "tags": n.tags,
                                })
                            })
                            .collect();
                        println!("{}", serde_json::to_string_pretty(&json_nodes).unwrap());
                    } else {
                        for node in filtered {
                            if let Some(ref res) = node.resolution {
                                let status_str = format!("[{:?}]", res.status).to_lowercase();
                                if blocked || deferred {
                                    let reason = res.reason.as_deref().unwrap_or("");
                                    println!(
                                        "{} {} - {} {}",
                                        node.id.cyan(),
                                        status_str.yellow(),
                                        node.title,
                                        reason.dimmed()
                                    );
                                } else {
                                    println!(
                                        "{} {} - {}",
                                        node.id.cyan(),
                                        status_str.yellow(),
                                        node.title
                                    );
                                }
                            } else {
                                println!("{} - {}", node.id.cyan(), node.title);
                            }
                        }
                    }
                }
                Err(e) => emit_error(&format, "load_error", &e.to_string()),
            }
        }

        Commands::Resolve {
            id,
            verified,
            blocked,
            deferred,
            wontfix,
            format,
        } => {
            let root = get_lattice_root();

            let (resolution, reason) = if verified {
                (Resolution::Verified, None)
            } else if let Some(reason) = blocked {
                (Resolution::Blocked, Some(reason))
            } else if let Some(reason) = deferred {
                (Resolution::Deferred, Some(reason))
            } else if let Some(reason) = wontfix {
                (Resolution::Wontfix, Some(reason))
            } else {
                emit_error(
                    &format,
                    "missing_status",
                    "Must specify one of: --verified, --blocked, --deferred, --wontfix",
                );
            };

            let resolved_by = format!("agent:claude-{}", chrono::Utc::now().format("%Y-%m-%d"));

            let options = ResolveOptions {
                node_id: id.clone(),
                resolution: resolution.clone(),
                reason: reason.clone(),
                resolved_by,
            };

            match resolve_node(&root, options) {
                Ok(path) => {
                    let status_str = format!("{:?}", resolution).to_lowercase();
                    if is_json(&format) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "success": true,
                                "id": id,
                                "resolution": status_str,
                                "reason": reason,
                                "file": path.display().to_string(),
                            }))
                            .unwrap()
                        );
                    } else {
                        println!("{}", format!("Resolved {} as {}", id, status_str).green());
                        if let Some(r) = reason {
                            println!("{}", format!("Reason: {}", r).dimmed());
                        }
                        println!("{}", format!("File: {}", path.display()).dimmed());
                    }
                }
                Err(e) => emit_error(&format, "resolve_error", &e.to_string()),
            }
        }

        Commands::Edit {
            id,
            title,
            body,
            status,
            priority,
            tags,
            category,
            files,
            test_command,
            format,
        } => {
            let root = get_lattice_root();
            let tags = split_csv(tags);
            let files = split_csv(files);
            let status = status.map(|s| parse_status(&s));
            let priority = priority.map(|p| parse_priority(&p));

            let options = EditNodeOptions {
                node_id: id.clone(),
                title,
                body,
                status,
                priority,
                tags,
                category,
                files,
                test_command,
            };

            match edit_node(&root, options) {
                Ok(path) => {
                    if is_json(&format) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "success": true,
                                "id": id,
                                "file": path.display().to_string(),
                            }))
                            .unwrap()
                        );
                    } else {
                        println!("{}", format!("Updated {}", id).green());
                        println!("{}", format!("File: {}", path.display()).dimmed());
                    }
                }
                Err(e) => emit_error(&format, "edit_error", &e.to_string()),
            }
        }

        Commands::Plan { requirements } => {
            let root = get_lattice_root();

            match build_node_index(&root) {
                Ok(index) => {
                    let plan = generate_plan(&requirements, &index);
                    print_plan(&plan, &index);
                }
                Err(e) => {
                    eprintln!("{}", format!("Error: {}", e).red());
                    process::exit(1);
                }
            }
        }

        Commands::Drift {
            check,
            acknowledge,
            format,
        } => {
            let root = get_lattice_root();

            // Handle --acknowledge
            if let Some(node_id) = acknowledge {
                match lattice::storage::acknowledge_drift(&root, &node_id) {
                    Ok(path) => {
                        if is_json(&format) {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&json!({
                                    "acknowledged": true,
                                    "node_id": node_id,
                                    "path": path.display().to_string(),
                                }))
                                .unwrap()
                            );
                        } else {
                            println!(
                                "{} Edge versions re-snapshotted on {}",
                                "Drift acknowledged.".green(),
                                node_id.cyan()
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                        process::exit(1);
                    }
                }
                return;
            }

            match find_drift(&root) {
                Ok(reports) => {
                    if is_json(&format) {
                        let json_reports: Vec<_> = reports
                            .iter()
                            .map(|r| {
                                let items: Vec<_> = r
                                    .drift_items
                                    .iter()
                                    .map(|i| {
                                        json!({
                                            "target_id": i.target_id,
                                            "bound_version": i.bound_version,
                                            "current_version": i.current_version,
                                            "severity": format!("{:?}", i.severity).to_lowercase(),
                                        })
                                    })
                                    .collect();
                                json!({
                                    "node_id": r.node_id,
                                    "node_type": r.node_type,
                                    "items": items,
                                })
                            })
                            .collect();
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "has_drift": !reports.is_empty(),
                                "count": reports.len(),
                                "reports": json_reports,
                            }))
                            .unwrap()
                        );
                        if check && !reports.is_empty() {
                            process::exit(2);
                        }
                    } else {
                        if reports.is_empty() {
                            println!("{}", "No drift detected".green());
                            return;
                        }

                        println!(
                            "{}",
                            format!("DRIFT DETECTED ({} nodes):\n", reports.len()).yellow()
                        );

                        for report in &reports {
                            println!(
                                "{}",
                                format!("{} ({})", report.node_id, report.node_type).cyan()
                            );
                            for item in &report.drift_items {
                                let severity_str = match item.severity {
                                    DriftSeverity::Major => "[major]".red(),
                                    DriftSeverity::Minor => "[minor]".yellow(),
                                    DriftSeverity::Patch => "[patch]".dimmed(),
                                };
                                println!(
                                    "  -> {}: {} -> {} {}",
                                    item.target_id,
                                    item.bound_version,
                                    item.current_version,
                                    severity_str
                                );
                            }
                            println!();
                        }

                        if check {
                            process::exit(2);
                        }
                    }
                }
                Err(e) => emit_error(&format, "drift_error", &e.to_string()),
            }
        }

        Commands::Get { id, format } => {
            let root = get_lattice_root();

            match build_node_index(&root) {
                Ok(index) => {
                    if let Some(node) = index.get(&id) {
                        if is_json(&format) {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&node)
                                    .unwrap_or_else(|_| "{}".to_string())
                            );
                        } else {
                            println!(
                                "{}",
                                format!("{} ({:?})", node.id, node.node_type)
                                    .to_lowercase()
                                    .cyan()
                            );
                            println!("{}", node.title.bold());
                            println!();
                            println!("{}", node.body);
                            println!();

                            // Show resolution if present
                            if let Some(ref res) = node.resolution {
                                let res_text = if let Some(ref reason) = res.reason {
                                    format!("Resolution: {:?} ({})", res.status, reason)
                                        .to_lowercase()
                                } else {
                                    format!("Resolution: {:?}", res.status).to_lowercase()
                                };
                                println!("{}", res_text.yellow());
                            }

                            // Show edge summary if edges exist
                            if let Some(ref edges) = node.edges
                                && let Some(summary) = summarize_edges(edges)
                            {
                                println!("{}", summary.dimmed());
                            }

                            println!(
                                "{}",
                                format!("Status: {:?} | Version: {}", node.status, node.version)
                                    .to_lowercase()
                                    .dimmed()
                            );
                        }
                    } else {
                        emit_error(
                            &format,
                            "node_not_found",
                            &format!("Node not found: {}", id),
                        );
                    }
                }
                Err(e) => emit_error(&format, "load_error", &e.to_string()),
            }
        }

        Commands::Export {
            format,
            audience,
            title,
            include_internal,
            output,
        } => {
            let root = get_lattice_root();

            if format == "json" {
                match build_node_index(&root) {
                    Ok(index) => {
                        let config = load_config(&root);
                        let nodes: Vec<_> = index.values().collect();
                        let output = json!({
                            "project": config.project,
                            "description": config.description,
                            "generated_at": chrono::Utc::now().to_rfc3339(),
                            "nodes": nodes,
                        });
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&output)
                                .unwrap_or_else(|_| "{}".to_string())
                        );
                    }
                    Err(e) => {
                        eprintln!("{}", format!("Error: {}", e).red());
                        process::exit(1);
                    }
                }
                return;
            }

            if format == "pages" {
                let output_dir = output.unwrap_or_else(|| "_site".to_string());

                // Derive GitHub Pages URL from git remote
                let pages_base = match get_github_pages_url() {
                    Some(url) => url,
                    None => {
                        eprintln!(
                            "{}",
                            "Error: Could not derive GitHub Pages URL from git remote. Ensure a GitHub origin is configured."
                                .red()
                        );
                        process::exit(1);
                    }
                };

                let json_url = format!("{}/lattice-data.json", pages_base);
                let reader_url = format!("https://forkzero.ai/reader?url={}", json_url);

                // Build JSON export
                match build_node_index(&root) {
                    Ok(index) => {
                        let config = load_config(&root);
                        let nodes: Vec<_> = index.values().collect();
                        let json_output = json!({
                            "project": config.project,
                            "description": config.description,
                            "generated_at": chrono::Utc::now().to_rfc3339(),
                            "nodes": nodes,
                        });

                        // Create output directory
                        if let Err(e) = std::fs::create_dir_all(&output_dir) {
                            eprintln!("{}", format!("Error creating {}: {}", output_dir, e).red());
                            process::exit(1);
                        }

                        // Write lattice-data.json
                        let json_path = std::path::Path::new(&output_dir).join("lattice-data.json");
                        let json_str = serde_json::to_string_pretty(&json_output)
                            .unwrap_or_else(|_| "{}".to_string());
                        if let Err(e) = std::fs::write(&json_path, &json_str) {
                            eprintln!(
                                "{}",
                                format!("Error writing {}: {}", json_path.display(), e).red()
                            );
                            process::exit(1);
                        }

                        // Write redirect index.html
                        let project_name = if config.project.is_empty() {
                            "Lattice".to_string()
                        } else {
                            config.project
                        };
                        let html = format!(
                            r#"<!DOCTYPE html>
<html><head>
  <meta charset="utf-8">
  <meta http-equiv="refresh" content="0;url={reader_url}">
  <title>{project_name} - Lattice</title>
</head><body>
  <p>Redirecting to <a href="{reader_url}">{project_name} Lattice Documentation</a>...</p>
</body></html>
"#
                        );
                        let html_path = std::path::Path::new(&output_dir).join("index.html");
                        if let Err(e) = std::fs::write(&html_path, &html) {
                            eprintln!(
                                "{}",
                                format!("Error writing {}: {}", html_path.display(), e).red()
                            );
                            process::exit(1);
                        }

                        eprintln!("{}", format!("Pages exported to {}/", output_dir).green());
                        eprintln!("  {} lattice-data.json", "✓".green());
                        eprintln!("  {} index.html → {}", "✓".green(), reader_url);
                    }
                    Err(e) => {
                        eprintln!("{}", format!("Error: {}", e).red());
                        process::exit(1);
                    }
                }
                return;
            }

            if format == "html" {
                let output_dir = output.unwrap_or_else(|| "_site".to_string());

                let sources = load_nodes_by_type(&root, "sources").unwrap_or_default();
                let theses = load_nodes_by_type(&root, "theses").unwrap_or_default();
                let requirements = load_nodes_by_type(&root, "requirements").unwrap_or_default();
                let implementations =
                    load_nodes_by_type(&root, "implementations").unwrap_or_default();

                let messages = load_nodes_by_type(&root, "messages").unwrap_or_default();

                let data = LatticeData {
                    sources,
                    theses,
                    requirements,
                    implementations,
                    messages,
                };

                let options = HtmlExportOptions {
                    output_dir: std::path::PathBuf::from(&output_dir),
                    title,
                };

                match export_html(&data, &options) {
                    Ok(path) => {
                        println!("{}", format!("HTML exported to {}", path.display()).green());
                    }
                    Err(e) => {
                        eprintln!("{}", format!("Error: {}", e).red());
                        process::exit(1);
                    }
                }
                return;
            }

            if format == "narrative" {
                let audience: Audience = match audience.parse() {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("{}", e.red());
                        process::exit(1);
                    }
                };

                let sources = load_nodes_by_type(&root, "sources").unwrap_or_default();
                let theses = load_nodes_by_type(&root, "theses").unwrap_or_default();
                let requirements = load_nodes_by_type(&root, "requirements").unwrap_or_default();
                let implementations =
                    load_nodes_by_type(&root, "implementations").unwrap_or_default();
                let messages = load_nodes_by_type(&root, "messages").unwrap_or_default();

                let data = LatticeData {
                    sources,
                    theses,
                    requirements,
                    implementations,
                    messages,
                };

                let options = ExportOptions {
                    audience,
                    title,
                    include_internal,
                };

                println!("{}", export_narrative(&data, &options));
                return;
            }

            eprintln!("{}", format!("Unknown format: {}", format).red());
            process::exit(1);
        }

        Commands::Summary { format } => {
            let root = get_lattice_root();

            let sources = load_nodes_by_type(&root, "sources").unwrap_or_default();
            let theses = load_nodes_by_type(&root, "theses").unwrap_or_default();
            let requirements = load_nodes_by_type(&root, "requirements").unwrap_or_default();
            let implementations = load_nodes_by_type(&root, "implementations").unwrap_or_default();
            let messages = load_nodes_by_type(&root, "messages").unwrap_or_default();

            // Count contested theses
            let contested_count = theses
                .iter()
                .filter(|t| t.status == Status::Contested)
                .count();

            // Count requirements by resolution status
            let mut unresolved = 0;
            let mut verified = 0;
            let mut blocked = 0;
            let mut deferred = 0;
            let mut wontfix = 0;

            // Count by priority
            let mut p0 = 0;
            let mut p1 = 0;
            let mut p2 = 0;

            // Track orphaned requirements (no derives_from)
            let mut orphaned_reqs: Vec<String> = Vec::new();

            for req in &requirements {
                // Resolution status
                match req.resolution.as_ref().map(|r| &r.status) {
                    Some(Resolution::Verified) => verified += 1,
                    Some(Resolution::Blocked) => blocked += 1,
                    Some(Resolution::Deferred) => deferred += 1,
                    Some(Resolution::Wontfix) => wontfix += 1,
                    None => unresolved += 1,
                }

                // Priority
                match req.priority {
                    Some(Priority::P0) => p0 += 1,
                    Some(Priority::P1) => p1 += 1,
                    Some(Priority::P2) => p2 += 1,
                    None => {}
                }

                // Check for orphaned (no derives_from)
                let has_derives_from = req
                    .edges
                    .as_ref()
                    .and_then(|e| e.derives_from.as_ref())
                    .map(|d| !d.is_empty())
                    .unwrap_or(false);
                if !has_derives_from {
                    orphaned_reqs.push(req.id.clone());
                }
            }

            // Check for drift
            let drift_reports = find_drift(&root).unwrap_or_default();
            let has_drift = !drift_reports.is_empty();

            // Check for orphaned theses (no requirements derive from them)
            let thesis_ids: std::collections::HashSet<_> =
                theses.iter().map(|t| t.id.clone()).collect();
            let mut referenced_theses: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for req in &requirements {
                if let Some(edges) = &req.edges
                    && let Some(derives_from) = &edges.derives_from
                {
                    for edge in derives_from {
                        referenced_theses.insert(edge.target.clone());
                    }
                }
            }
            let orphaned_theses: Vec<_> =
                thesis_ids.difference(&referenced_theses).cloned().collect();

            if format == "json" {
                let summary = serde_json::json!({
                    "nodes": {
                        "sources": sources.len(),
                        "theses": theses.len(),
                        "requirements": requirements.len(),
                        "implementations": implementations.len(),
                        "messages": messages.len()
                    },
                    "contested_theses": contested_count,
                    "requirements": {
                        "by_resolution": {
                            "unresolved": unresolved,
                            "verified": verified,
                            "blocked": blocked,
                            "deferred": deferred,
                            "wontfix": wontfix
                        },
                        "by_priority": {
                            "P0": p0,
                            "P1": p1,
                            "P2": p2
                        },
                        "orphaned": orphaned_reqs
                    },
                    "theses": {
                        "orphaned": orphaned_theses
                    },
                    "drift": {
                        "has_drift": has_drift,
                        "count": drift_reports.len()
                    }
                });
                println!("{}", serde_json::to_string_pretty(&summary).unwrap());
            } else {
                // Text format
                println!("{}", "Lattice Summary".cyan().bold());
                println!();

                println!("{}", "Nodes:".bold());
                println!(
                    "  {} sources, {} theses, {} requirements, {} implementations, {} messages",
                    sources.len(),
                    theses.len(),
                    requirements.len(),
                    implementations.len(),
                    messages.len()
                );
                if contested_count > 0 {
                    println!(
                        "  {}",
                        format!("{} contested theses", contested_count).yellow()
                    );
                }
                println!();

                println!("{}", "Requirements by resolution:".bold());
                println!(
                    "  {} unresolved, {} verified, {} blocked, {} deferred, {} wontfix",
                    unresolved, verified, blocked, deferred, wontfix
                );
                println!();

                println!("{}", "Requirements by priority:".bold());
                println!("  {} P0, {} P1, {} P2", p0, p1, p2);
                println!();

                if has_drift {
                    println!(
                        "{}",
                        format!("Drift: {} stale edges detected", drift_reports.len()).yellow()
                    );
                } else {
                    println!("{}", "Drift: none".green());
                }
                println!();

                if !orphaned_reqs.is_empty() {
                    println!(
                        "{}",
                        format!(
                            "Orphaned requirements (no derives_from): {}",
                            orphaned_reqs.len()
                        )
                        .yellow()
                    );
                    for id in &orphaned_reqs {
                        println!("  - {}", id);
                    }
                    println!();
                }

                if !orphaned_theses.is_empty() {
                    println!(
                        "{}",
                        format!(
                            "Orphaned theses (no requirements derive from them): {}",
                            orphaned_theses.len()
                        )
                        .yellow()
                    );
                    for id in &orphaned_theses {
                        println!("  - {}", id);
                    }
                }
            }
        }

        Commands::Health {
            check,
            strict,
            format,
        } => {
            let root = get_lattice_root();

            // Check if .lattice/ has staged changes (index-aware for pre-commit, #31)
            let lattice_staged = std::process::Command::new("git")
                .args(["diff", "--cached", "--quiet", "--", ".lattice/"])
                .current_dir(&root)
                .output()
                .ok()
                .map(|o| !o.status.success()) // exit 1 = staged changes exist
                .unwrap_or(false);

            // 1. Freshness: single git call for lattice commit hash + timestamp
            let lattice_git = std::process::Command::new("git")
                .args(["log", "-1", "--format=%H %ct", "--", ".lattice/"])
                .current_dir(&root)
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            let (lattice_commit, lattice_ts) = {
                let parts: Vec<&str> = lattice_git.split_whitespace().collect();
                if parts.len() == 2 {
                    (parts[0].to_string(), parts[1].parse::<i64>().ok())
                } else {
                    (String::new(), None)
                }
            };
            let code_ts = std::process::Command::new("git")
                .args([
                    "log",
                    "-1",
                    "--format=%ct",
                    "--",
                    ".",
                    ":!.lattice/",
                    ":!.claude/",
                    ":!docs/",
                    ":!README.md",
                    ":!CLAUDE.md",
                    ":!LICENSE",
                ])
                .current_dir(&root)
                .output()
                .ok()
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .parse::<i64>()
                        .ok()
                });
            // If .lattice/ has staged changes, this commit IS the catch-up (#31)
            let freshness_gap_hours = if lattice_staged {
                0
            } else {
                match (lattice_ts, code_ts) {
                    (Some(l), Some(c)) if c > l => (c - l) as u64 / 3600,
                    _ => 0,
                }
            };

            // 2. Load graph once — derive all metrics from the index
            let index = build_node_index(&root).unwrap_or_default();
            let all_nodes: Vec<_> = index.values().collect();

            let contested_count = all_nodes
                .iter()
                .filter(|n| n.node_type == NodeType::Thesis && n.status == Status::Contested)
                .count();

            let mut drift_count = 0;
            for node in &all_nodes {
                for edge in node.all_edges() {
                    if let Some(target) = index.get(&edge.target)
                        && edge.version_or_default() != target.version
                    {
                        drift_count += 1;
                    }
                }
            }

            let change_pressure = contested_count + drift_count;

            // 3. Code impact: files changed since last .lattice/ commit that are bound in implementations
            let bound_files: std::collections::HashSet<String> = all_nodes
                .iter()
                .filter(|n| n.node_type == NodeType::Implementation)
                .filter_map(|n| {
                    if let Some(NodeMeta::Implementation(meta)) = &n.meta {
                        meta.files.as_ref()
                    } else {
                        None
                    }
                })
                .flat_map(|files| files.iter().map(|f| f.path.clone()))
                .collect();

            // If lattice is staged, this commit resolves the diff coupling (#31)
            let (tracked_files_changed, total_files_changed, affected_file_names) =
                if lattice_staged {
                    (0, 0, Vec::new())
                } else if !lattice_commit.is_empty() {
                    let output = std::process::Command::new("git")
                        .args([
                            "diff",
                            "--name-only",
                            &lattice_commit,
                            "--",
                            ".",
                            ":!.lattice/",
                        ])
                        .current_dir(&root)
                        .output()
                        .ok()
                        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                        .unwrap_or_default();
                    let changed: Vec<String> = output
                        .lines()
                        .filter(|l| !l.is_empty())
                        .map(|s| s.to_string())
                        .collect();
                    let affected: Vec<String> = changed
                        .iter()
                        .filter(|f| bound_files.contains(f.as_str()))
                        .cloned()
                        .collect();
                    let tracked = affected.len();
                    let total = changed.len();
                    (tracked, total, affected)
                } else {
                    (0, 0, Vec::new())
                };

            // 4. Lint (strict mode only)
            let lint_issues = if strict {
                lint_lattice(&root).issues.len()
            } else {
                0
            };

            // Read configurable threshold
            let config = load_config(&root);
            let threshold = config.freshness_threshold_hours.unwrap_or(72);

            // Compute verdict:
            // Strict mode: FAIL on diff-coupled staleness (bound files changed without lattice update)
            //              and lint issues. Wall-clock staleness alone stays WARN.
            // Normal mode: FAIL when pressure + signals combine. WARN on any single signal.
            let verdict = if (strict && (lint_issues > 0 || tracked_files_changed > 0))
                || (change_pressure > 0
                    && (tracked_files_changed > 0 || freshness_gap_hours > threshold))
                || change_pressure > 3
            {
                "FAIL"
            } else if tracked_files_changed > 0
                || freshness_gap_hours > threshold
                || change_pressure > 0
            {
                "WARN"
            } else {
                "PASS"
            };

            if is_json(&format) {
                let mut result = json!({
                    "verdict": verdict,
                    "freshness": {
                        "gap_hours": freshness_gap_hours,
                    },
                    "change_pressure": {
                        "contested_theses": contested_count,
                        "drift_items": drift_count,
                        "total": change_pressure,
                    },
                    "code_impact": {
                        "total_files_changed": total_files_changed,
                        "tracked_files_changed": tracked_files_changed,
                        "bound_files_count": bound_files.len(),
                        "affected_files": affected_file_names,
                    },
                    "lattice_staged": lattice_staged,
                });
                if strict {
                    result["lint"] = json!({ "issues": lint_issues });
                }
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
            } else {
                let verdict_colored = match verdict {
                    "PASS" => "PASS".green().bold(),
                    "WARN" => "WARN".yellow().bold(),
                    _ => "FAIL".red().bold(),
                };
                println!("{} {}\n", "HEALTH:".bold(), verdict_colored);

                println!("  {}", "Freshness:".bold());
                if lattice_staged {
                    println!("    {}", "Lattice update staged — credited".green());
                } else if freshness_gap_hours > 0 {
                    println!(
                        "    Lattice is {}h behind code changes",
                        freshness_gap_hours
                    );
                } else {
                    println!("    {}", "Lattice is up to date".green());
                }
                println!();

                println!("  {}", "Change Pressure:".bold());
                println!("    Contested theses: {}", contested_count);
                println!("    Drift items: {}", drift_count);
                println!();

                println!("  {}", "Code Impact:".bold());
                if lattice_staged {
                    println!(
                        "    {}",
                        "Lattice update staged — diff coupling cleared".green()
                    );
                } else {
                    println!(
                        "    {} files changed since last lattice update",
                        total_files_changed
                    );
                    if !bound_files.is_empty() {
                        println!(
                            "    {} of {} lattice-tracked files affected",
                            tracked_files_changed,
                            bound_files.len()
                        );
                    }
                    for f in &affected_file_names {
                        println!("      {}", f.yellow());
                    }
                }

                if strict {
                    println!();
                    println!("  {}", "Lint:".bold());
                    if lint_issues > 0 {
                        println!("    {}", format!("{} issues found", lint_issues).red());
                    } else {
                        println!("    {}", "No issues".green());
                    }
                }
            }

            // Print remediation hint on FAIL (text mode only)
            if verdict == "FAIL" && !is_json(&format) {
                println!();
                if tracked_files_changed > 0 && !lattice_staged {
                    println!(
                        "  {}",
                        "Tip: Re-verify bound implementations (lattice verify/edit),".dimmed()
                    );
                    println!(
                        "  {}",
                        "  then stage .lattice/ alongside your code: git add .lattice/".dimmed()
                    );
                } else if lint_issues > 0 {
                    println!(
                        "  {}",
                        "Tip: Run 'lattice lint --fix' to auto-fix, or address manually.".dimmed()
                    );
                }
            }

            if check && verdict == "FAIL" {
                process::exit(2);
            }
        }

        Commands::Lint {
            fix,
            strict,
            format,
        } => {
            let root = get_lattice_root();
            let report = lint_lattice(&root);

            if is_json(&format) {
                let json_issues: Vec<_> = report
                    .issues
                    .iter()
                    .map(|i| {
                        json!({
                            "file": i.file.display().to_string(),
                            "node_id": i.node_id,
                            "severity": format!("{}", i.severity),
                            "message": i.message,
                            "fixable": i.fixable == lattice::lint::Fixable::Yes,
                        })
                    })
                    .collect();

                let mut fixed_msgs = Vec::new();
                if fix && !report.fixable().is_empty() {
                    fixed_msgs = fix_issues(&root, &report);
                }

                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "errors": report.errors().len(),
                        "warnings": report.warnings().len(),
                        "fixable": report.fixable().len(),
                        "issues": json_issues,
                        "fixed": fixed_msgs,
                    }))
                    .unwrap()
                );

                if report.has_errors() {
                    process::exit(1);
                } else if strict && !report.issues.is_empty() {
                    process::exit(2);
                }
            } else {
                if report.issues.is_empty() {
                    println!("{}", "No issues found".green());
                    return;
                }

                let errors = report.errors();
                let warnings = report.warnings();

                for issue in &report.issues {
                    let colored_msg = match issue.severity {
                        LintSeverity::Error => format!("{}", issue).red().to_string(),
                        LintSeverity::Warning => format!("{}", issue).yellow().to_string(),
                    };
                    println!("{}", colored_msg);
                }
                println!();

                println!(
                    "{}",
                    format!(
                        "{} error(s), {} warning(s), {} fixable",
                        errors.len(),
                        warnings.len(),
                        report.fixable().len()
                    )
                    .bold()
                );

                if fix && !report.fixable().is_empty() {
                    println!();
                    let fixed = fix_issues(&root, &report);
                    for msg in &fixed {
                        println!("{}", format!("Fixed: {}", msg).green());
                    }
                    println!(
                        "{}",
                        format!("{} issue(s) fixed", fixed.len()).green().bold()
                    );
                }

                if strict || report.has_errors() {
                    process::exit(1);
                }
            }
        }

        Commands::Freshness {
            threshold,
            check,
            format,
        } => {
            let root = get_lattice_root();

            // Index-aware: if .lattice/ has staged changes, credit this commit (#31)
            let lattice_staged = std::process::Command::new("git")
                .args(["diff", "--cached", "--quiet", "--", ".lattice/"])
                .current_dir(&root)
                .output()
                .ok()
                .map(|o| !o.status.success())
                .unwrap_or(false);

            if lattice_staged {
                if is_json(&format) {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({
                            "stale": false,
                            "lattice_staged": true,
                            "gap_hours": 0,
                            "threshold_hours": threshold,
                        }))
                        .unwrap()
                    );
                } else {
                    println!(
                        "{}",
                        "Fresh: lattice update staged — credited for this commit".green()
                    );
                }
                return;
            }

            // Get the most recent commit timestamp touching .lattice/ files
            let lattice_ts = std::process::Command::new("git")
                .args(["log", "-1", "--format=%ct", "--", ".lattice/"])
                .current_dir(&root)
                .output()
                .ok()
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .parse::<i64>()
                        .ok()
                });

            // Get the most recent commit timestamp touching code files (exclude .lattice/)
            let code_ts = std::process::Command::new("git")
                .args([
                    "log",
                    "-1",
                    "--format=%ct",
                    "--",
                    ".",
                    ":!.lattice/",
                    ":!.claude/",
                    ":!docs/",
                    ":!README.md",
                    ":!CLAUDE.md",
                    ":!LICENSE",
                ])
                .current_dir(&root)
                .output()
                .ok()
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .parse::<i64>()
                        .ok()
                });

            match (lattice_ts, code_ts) {
                (Some(lattice), Some(code)) => {
                    let gap_hours = (code - lattice).max(0) as u64 / 3600;
                    let stale = code > lattice && gap_hours >= threshold;

                    let lattice_dt = chrono::DateTime::from_timestamp(lattice, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let code_dt = chrono::DateTime::from_timestamp(code, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    if is_json(&format) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "stale": stale,
                                "lattice_last_updated": lattice_dt,
                                "code_last_updated": code_dt,
                                "gap_hours": gap_hours,
                                "threshold_hours": threshold,
                            }))
                            .unwrap()
                        );
                    } else if stale {
                        println!(
                            "{}",
                            format!(
                                "STALE: Code updated {} but lattice last updated {} ({} hours ago, threshold: {}h)",
                                code_dt, lattice_dt, gap_hours, threshold
                            )
                            .yellow()
                        );
                    } else {
                        println!(
                            "{}",
                            format!(
                                "Fresh: lattice updated {} — within {}h threshold",
                                lattice_dt, threshold
                            )
                            .green()
                        );
                    }

                    if check && stale {
                        process::exit(2);
                    }
                }
                (None, Some(_)) => {
                    let msg = "No lattice commits found — lattice has never been updated";
                    if is_json(&format) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "stale": true,
                                "error": msg,
                            }))
                            .unwrap()
                        );
                    } else {
                        println!("{}", msg.yellow());
                    }
                    if check {
                        process::exit(2);
                    }
                }
                (Some(lattice), None) => {
                    let lattice_dt = chrono::DateTime::from_timestamp(lattice, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let msg = format!("No code commits found — lattice updated {}", lattice_dt);
                    if is_json(&format) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "stale": false,
                                "lattice_last_updated": lattice_dt,
                            }))
                            .unwrap()
                        );
                    } else {
                        println!("{}", msg.green());
                    }
                }
                (None, None) => {
                    let msg = "No git history found";
                    if is_json(&format) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "stale": false,
                                "error": msg,
                            }))
                            .unwrap()
                        );
                    } else {
                        println!("{}", msg);
                    }
                }
            }
        }

        Commands::Assess { check, format } => {
            let root = get_lattice_root();
            let all_nodes = load_all_nodes(&root).unwrap_or_default();

            let contested_theses: Vec<_> = all_nodes
                .iter()
                .filter(|n| n.node_type == NodeType::Thesis && n.status == Status::Contested)
                .collect();

            let theses_with_history: Vec<_> = all_nodes
                .iter()
                .filter(|n| {
                    if let Some(NodeMeta::Thesis(meta)) = &n.meta {
                        !meta.confidence_history.is_empty()
                    } else {
                        false
                    }
                })
                .collect();

            // Find requirements downstream of contested theses
            let contested_ids: std::collections::HashSet<_> =
                contested_theses.iter().map(|n| n.id.as_str()).collect();
            let affected_requirements: Vec<_> = all_nodes
                .iter()
                .filter(|n| {
                    n.node_type == NodeType::Requirement
                        && n.edges.as_ref().is_some_and(|e| {
                            e.derives_from.as_ref().is_some_and(|refs| {
                                refs.iter()
                                    .any(|r| contested_ids.contains(r.target.as_str()))
                            })
                        })
                })
                .collect();

            // Use build_node_index to avoid loading all nodes a second time via find_drift
            let drift_count = build_node_index(&root)
                .ok()
                .map(|index| {
                    let mut count = 0;
                    for node in index.values() {
                        for edge in node.all_edges() {
                            if let Some(target) = index.get(&edge.target)
                                && edge.version_or_default() != target.version
                            {
                                count += 1;
                            }
                        }
                    }
                    count
                })
                .unwrap_or(0);

            let message_count = all_nodes
                .iter()
                .filter(|n| n.node_type == NodeType::Message)
                .count();

            let change_pressure = contested_theses.len()
                + theses_with_history.len()
                + affected_requirements.len()
                + drift_count;

            if is_json(&format) {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "contested_theses": contested_theses.len(),
                        "theses_with_confidence_changes": theses_with_history.len(),
                        "affected_requirements": affected_requirements.len(),
                        "drift_items": drift_count,
                        "messages": message_count,
                        "change_pressure": change_pressure,
                    }))
                    .unwrap()
                );
            } else {
                println!("{}", "CHANGE PRESSURE ASSESSMENT".bold());
                println!();
                println!(
                    "  {:<36} {}",
                    "Contested theses:".cyan(),
                    contested_theses.len()
                );
                println!(
                    "  {:<36} {}",
                    "Theses with confidence changes:".cyan(),
                    theses_with_history.len()
                );
                println!(
                    "  {:<36} {}",
                    "Requirements affected by contested:".cyan(),
                    affected_requirements.len()
                );
                println!("  {:<36} {}", "Drift items:".cyan(), drift_count);
                println!("  {:<36} {}", "Messages:".cyan(), message_count);
                println!();
                if change_pressure > 0 {
                    println!(
                        "{}",
                        format!(
                            "Change pressure: {} (contested: {}, drift: {})",
                            change_pressure,
                            contested_theses.len(),
                            drift_count
                        )
                        .yellow()
                    );
                } else {
                    println!("{}", "Change pressure: 0 — graph is stable".green());
                }
            }

            if check && change_pressure > 0 {
                process::exit(2);
            }
        }

        Commands::Migrate => {
            let root = get_lattice_root();

            // Create messages directory if missing
            let messages_dir = root.join(LATTICE_DIR).join("messages");
            if !messages_dir.exists() {
                fs::create_dir_all(&messages_dir).ok();
                println!("{}", "Created .lattice/messages/ directory".green());
            }

            // Migrate theses: add confidence_history if missing
            let theses = load_nodes_by_type(&root, "theses").unwrap_or_default();
            let mut migrated = 0;
            for node in &theses {
                if let Ok(path) = find_node_path(&root, &node.id) {
                    let contents = fs::read_to_string(&path).unwrap_or_default();
                    if !contents.contains("confidence_history") {
                        // Re-save to include new fields with defaults
                        let updated = node.clone();
                        if let Ok(yaml) = serde_yaml::to_string(&updated) {
                            fs::write(&path, yaml).ok();
                            migrated += 1;
                        }
                    }
                }
            }

            if migrated > 0 {
                println!(
                    "{}",
                    format!("Migrated {} thesis files to v0.2.0 schema", migrated).green()
                );
            } else {
                println!("{}", "All theses already at v0.2.0 schema".green());
            }

            // Update schema_version in config.yaml
            let config_path = root.join(LATTICE_DIR).join("config.yaml");
            if let Ok(contents) = fs::read_to_string(&config_path) {
                let updated = if contents.contains("schema_version:") {
                    contents
                        .lines()
                        .map(|line| {
                            if line.starts_with("schema_version:") {
                                format!("schema_version: \"{}\"", CURRENT_SCHEMA_VERSION)
                            } else {
                                line.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    format!(
                        "{}\nschema_version: \"{}\"\n",
                        contents.trim_end(),
                        CURRENT_SCHEMA_VERSION
                    )
                };
                fs::write(&config_path, updated).ok();
                println!(
                    "{}",
                    format!("Updated schema_version to {}", CURRENT_SCHEMA_VERSION).green()
                );
            }
        }

        Commands::Verify {
            implementation,
            relation: _,
            requirement,
            tests_pass,
            coverage,
            files,
            format,
        } => {
            let root = get_lattice_root();

            let options = VerifyOptions {
                implementation_id: implementation.clone(),
                requirement_id: requirement.clone(),
                tests_pass,
                coverage,
                files: split_csv(files),
                verified_by: format!("agent:claude-{}", chrono::Utc::now().format("%Y-%m-%d")),
            };

            match verify_implementation(&root, options) {
                Ok(path) => {
                    if is_json(&format) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "success": true,
                                "implementation": implementation,
                                "requirement": requirement,
                                "file": path.display().to_string(),
                            }))
                            .unwrap()
                        );
                    } else {
                        println!(
                            "{}",
                            format!("Verified: {} satisfies {}", implementation, requirement)
                                .green()
                        );
                        println!("{}", format!("File: {}", path.display()).dimmed());
                    }
                }
                Err(e) => emit_error(&format, "verify_error", &e.to_string()),
            }
        }

        Commands::Refine {
            parent,
            gap_type,
            title,
            description,
            proposed,
            implementation,
            format,
        } => {
            let root = get_lattice_root();

            let gap: GapType = match gap_type.parse() {
                Ok(g) => g,
                Err(e) => emit_error(&format, "invalid_gap_type", &e),
            };

            let options = RefineOptions {
                parent_id: parent.clone(),
                gap_type: gap,
                title,
                description,
                proposed,
                implementation_id: implementation,
                created_by: format!("agent:claude-{}", chrono::Utc::now().format("%Y-%m-%d")),
            };

            match refine_requirement(&root, options) {
                Ok(result) => {
                    if is_json(&format) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "success": true,
                                "sub_requirement_id": result.sub_requirement_id,
                                "parent": parent,
                                "file": result.sub_requirement_path.display().to_string(),
                                "parent_updated": result.parent_updated,
                                "implementation_updated": result.implementation_updated,
                            }))
                            .unwrap()
                        );
                    } else {
                        println!(
                            "{}",
                            format!(
                                "Created sub-requirement: {} (refines {})",
                                result.sub_requirement_id, parent
                            )
                            .green()
                        );
                        println!(
                            "{}",
                            format!("File: {}", result.sub_requirement_path.display()).dimmed()
                        );
                        if result.parent_updated {
                            println!(
                                "{}",
                                format!("Updated {} with depends_on edge", parent).dimmed()
                            );
                        }
                        if result.implementation_updated {
                            println!(
                                "{}",
                                "Updated implementation with reveals_gap_in edge".dimmed()
                            );
                        }
                    }
                }
                Err(e) => emit_error(&format, "refine_error", &e.to_string()),
            }
        }

        Commands::Search {
            positional_type,
            node_type,
            query,
            priority,
            resolution,
            tag,
            tags,
            category,
            id_prefix,
            related_to,
            index,
            index_status,
            #[cfg(feature = "vector-search")]
            semantic,
            min_score,
            limit,
            format,
        } => {
            let root = get_lattice_root();
            let engine = SearchEngine::new(&root);

            // Handle --index: build/rebuild the search index
            if index {
                match engine.index_build() {
                    Ok((total, unchanged)) => {
                        let changed = total - unchanged;
                        if is_json(&format) {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&json!({
                                    "success": true,
                                    "total": total,
                                    "changed": changed,
                                    "unchanged": unchanged,
                                }))
                                .unwrap()
                            );
                        } else {
                            println!(
                                "{}",
                                format!(
                                    "Indexed {} nodes ({} changed, {} unchanged)",
                                    total, changed, unchanged
                                )
                                .green()
                            );
                        }
                    }
                    Err(e) => emit_error(&format, "index_error", &e),
                }
                return;
            }

            // Handle --index-status: show index health
            if index_status {
                match engine.index_status() {
                    Ok(status) => {
                        if is_json(&format) {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&json!({
                                    "exists": status.exists,
                                    "indexed": status.indexed,
                                    "stale": status.stale,
                                    "missing": status.missing,
                                    "total": status.total,
                                    "cache_dir": status.cache_dir.display().to_string(),
                                }))
                                .unwrap()
                            );
                        } else if !status.exists {
                            println!("{}", "No search index found.".yellow());
                            println!(
                                "Run {} to build the index ({} nodes).",
                                "lattice search --index".bold(),
                                status.total
                            );
                        } else {
                            let healthy = status.stale == 0 && status.missing == 0;
                            let health_str = if healthy {
                                "healthy".green().to_string()
                            } else {
                                "needs rebuild".yellow().to_string()
                            };
                            println!("Index status: {}", health_str);
                            println!("  Indexed: {}", status.indexed);
                            println!("  Stale:   {}", status.stale);
                            println!("  Missing: {}", status.missing);
                            println!("  Total:   {}", status.total);
                            println!("  Cache:   {}", status.cache_dir.display());
                        }
                    }
                    Err(e) => emit_error(&format, "index_status_error", &e),
                }
                return;
            }

            // Handle --semantic: hybrid search (keyword + semantic fused via RRF)
            #[cfg(feature = "vector-search")]
            if semantic {
                if query.is_none() {
                    emit_error(
                        &format,
                        "semantic_error",
                        "--semantic requires -q/--query to specify what to search for",
                    );
                }
                let provider = match lattice::search::FastEmbedProvider::new() {
                    Ok(p) => p,
                    Err(e) => emit_error(&format, "semantic_error", &e),
                };
                let hybrid_params = SearchParams {
                    node_type: Some(
                        positional_type
                            .or(node_type)
                            .unwrap_or_else(|| "requirements".to_string()),
                    ),
                    query,
                    priority,
                    resolution,
                    tag,
                    tags: split_csv(tags),
                    category,
                    id_prefix,
                    related_to,
                };
                match engine.hybrid_search(&hybrid_params, &provider) {
                    Ok(mut results) => {
                        // Apply --min-score
                        if let Some(threshold) = min_score {
                            results
                                .results
                                .retain(|r| r.score.unwrap_or(0.0) >= threshold);
                            results.count = results.results.len();
                        }
                        // Apply --limit
                        if let Some(max) = limit {
                            results.results.truncate(max);
                            results.count = results.results.len();
                        }
                        if is_json(&format) {
                            let json_results: Vec<_> = results
                                .results
                                .iter()
                                .map(|r| {
                                    json!({
                                        "id": r.id,
                                        "title": r.title,
                                        "score": r.score,
                                        "body": r.body,
                                        "version": r.version,
                                        "priority": r.priority,
                                        "resolution": r.resolution,
                                        "category": r.category,
                                        "tags": r.tags
                                    })
                                })
                                .collect();
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&json!({
                                    "count": json_results.len(),
                                    "hybrid": true,
                                    "results": json_results
                                }))
                                .unwrap()
                            );
                        } else {
                            println!(
                                "{}",
                                format!("Found {} hybrid results:", results.count).bold()
                            );
                            for r in &results.results {
                                let priority_str = r
                                    .priority
                                    .as_ref()
                                    .map(|p| format!("[{}]", p))
                                    .unwrap_or_default();
                                let resolution_str = r
                                    .resolution
                                    .as_ref()
                                    .map(|s| format!(" ({})", s))
                                    .unwrap_or_default();
                                println!(
                                    "  {} {:.4} {} {}{}",
                                    r.id.cyan(),
                                    r.score.unwrap_or(0.0),
                                    priority_str.yellow(),
                                    r.title,
                                    resolution_str.dimmed()
                                );
                            }
                        }
                    }
                    Err(e) => emit_error(&format, "semantic_error", &e),
                }
                return;
            }

            let params = SearchParams {
                node_type: Some(
                    positional_type
                        .or(node_type)
                        .unwrap_or_else(|| "requirements".to_string()),
                ),
                query,
                priority,
                resolution,
                tag,
                tags: split_csv(tags),
                category,
                id_prefix,
                related_to,
            };

            let mut results = match engine.search(&params) {
                Ok(r) => r,
                Err(e) => emit_error(&format, "search_error", &e),
            };

            // Apply --min-score if specified
            if let Some(threshold) = min_score {
                results
                    .results
                    .retain(|r| r.score.unwrap_or(0.0) >= threshold);
                results.count = results.results.len();
            }

            // Apply --limit if specified
            if let Some(max) = limit {
                results.results.truncate(max);
                results.count = results.results.len();
            }

            if is_json(&format) {
                let json_results: Vec<_> = results
                    .results
                    .iter()
                    .map(|r| {
                        let mut obj = json!({
                            "id": r.id,
                            "title": r.title,
                            "body": r.body,
                            "version": r.version,
                            "priority": r.priority,
                            "resolution": r.resolution,
                            "category": r.category,
                            "tags": r.tags
                        });
                        if let Some(score) = r.score {
                            obj["score"] = json!(score);
                        }
                        obj
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "count": json_results.len(),
                        "results": json_results
                    }))
                    .unwrap()
                );
            } else {
                println!("{}", format!("Found {} results:", results.count).bold());
                for r in &results.results {
                    let priority_str = r
                        .priority
                        .as_ref()
                        .map(|p| format!("[{}]", p))
                        .unwrap_or_default();
                    let resolution_str = r
                        .resolution
                        .as_ref()
                        .map(|s| format!(" ({})", s))
                        .unwrap_or_default();
                    let score_str = r.score.map(|s| format!("{:.1} ", s)).unwrap_or_default();
                    println!(
                        "  {} {}{} {}{}",
                        r.id.cyan(),
                        score_str,
                        priority_str.yellow(),
                        r.title,
                        resolution_str.dimmed()
                    );
                }
            }
        }

        Commands::Mcp => {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            if let Err(e) = rt.block_on(lattice::mcp::run_server()) {
                eprintln!("{}", format!("MCP server error: {}", e).red());
                process::exit(1);
            }
        }

        Commands::Update {
            check,
            force,
            version,
            format,
        } => {
            use lattice::update::{UpdateOptions, UpdateResult};

            if !lattice::update::is_installed_binary() {
                if is_json(&format) {
                    eprintln!(
                        "{}",
                        json!({"error": "not_installed", "detail": "Running from a development build. Use the install script or `cargo install` instead."})
                    );
                } else {
                    eprintln!(
                        "{}",
                        "Warning: running from a development build (target/). Update may not work as expected."
                            .yellow()
                    );
                }
            }

            if !check {
                if is_json(&format) {
                    println!(
                        "{}",
                        json!({"status": "checking", "current_version": env!("CARGO_PKG_VERSION"), "target": lattice::update::TARGET_TRIPLE})
                    );
                } else {
                    eprintln!("{}", "Checking for updates...".dimmed());
                }
            }

            let options = UpdateOptions {
                check_only: check,
                force,
                target_version: version,
            };

            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            match rt.block_on(lattice::update::run_update(options)) {
                Ok(UpdateResult::AlreadyUpToDate { version: v }) => {
                    if is_json(&format) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "status": "up_to_date",
                                "version": v.to_string()
                            }))
                            .unwrap()
                        );
                    } else {
                        println!(
                            "{}",
                            format!("lattice v{} is already up to date.", v).green()
                        );
                    }
                }
                Ok(UpdateResult::UpdateAvailable {
                    current: c,
                    latest: l,
                }) => {
                    if is_json(&format) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "status": "update_available",
                                "current_version": c.to_string(),
                                "latest_version": l.to_string()
                            }))
                            .unwrap()
                        );
                    } else {
                        println!("Current: v{}, Latest: v{}", c, l);
                        println!("{}", "Run `lattice update` to install.".dimmed());
                    }
                }
                Ok(UpdateResult::Updated { from, to }) => {
                    if is_json(&format) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "status": "updated",
                                "from_version": from.to_string(),
                                "to_version": to.to_string()
                            }))
                            .unwrap()
                        );
                    } else {
                        println!(
                            "{}",
                            format!("Updated lattice from v{} to v{}", from, to).green()
                        );
                    }
                }
                Err(e) => {
                    let (message, hint) = match &e {
                        lattice::update::UpdateError::Replace(inner)
                            if inner.contains("Permission denied") =>
                        {
                            let exe = std::env::current_exe()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|_| "lattice".to_string());
                            (
                                format!("Permission denied — cannot write to {}", exe),
                                if cfg!(unix) {
                                    Some(
                                        "Reinstall to ~/.local/bin for sudo-free updates:\n  INSTALL_DIR=~/.local/bin curl -fsSL https://forkzero.ai/lattice/install.sh | sh\nOr run: sudo lattice update",
                                    )
                                } else {
                                    None
                                },
                            )
                        }
                        _ => (e.to_string(), None),
                    };

                    if is_json(&format) {
                        let mut err = json!({"error": "update_failed", "detail": message});
                        if let Some(h) = hint {
                            err["hint"] = json!(h);
                        }
                        eprintln!("{}", err);
                    } else {
                        eprintln!("{}", format!("Error: {}", message).red());
                        if let Some(h) = hint {
                            eprintln!("{}", h.dimmed());
                        }
                    }
                    process::exit(1);
                }
            }
        }

        Commands::Prompt { mcp } => {
            if mcp {
                print!("{}", include_str!("../prompts/LATTICE_MCP_CLAUDE_MD.md"));
            } else {
                print!("{}", include_str!("../prompts/LATTICE_CLAUDE_MD.md"));
            }
            eprintln!();
            eprintln!(
                "{}",
                "Tip: Run `lattice init --skill` to auto-install as a Claude Code skill (recommended)."
                    .dimmed()
            );
        }

        Commands::Push {
            api_url,
            api_key,
            format,
        } => {
            let root = get_lattice_root();
            let config = load_config(&root);
            let (url, key) = resolve_api_credentials(api_url, api_key, &config, &format);

            let nodes = load_all_nodes(&root).unwrap_or_else(|e| {
                emit_error(
                    &format,
                    "load_failed",
                    &format!("Failed to load lattice: {}", e),
                );
            });

            // Get current git SHA
            let git_sha = lattice::diff::git_head_sha().unwrap_or_else(|e| {
                eprintln!(
                    "{}",
                    format!("Warning: could not get git SHA: {}", e).yellow()
                );
                "unknown".to_string()
            });

            if !is_json(&format) {
                println!(
                    "{}",
                    format!("Pushing {} nodes to {}...", nodes.len(), url).dimmed()
                );
            }

            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");

            // Fetch last push SHA and compute diff if available
            let push_diff = rt
                .block_on(lattice::push::fetch_last_push_sha(
                    &url,
                    &key,
                    &config.project,
                ))
                .and_then(|baseline_sha| {
                    if !is_json(&format) {
                        eprintln!(
                            "{}",
                            format!(
                                "Computing diff since {}...",
                                &baseline_sha[..std::cmp::min(8, baseline_sha.len())]
                            )
                            .dimmed()
                        );
                    }
                    match lattice::diff::lattice_diff(&root, Some(&baseline_sha)) {
                        Ok(d) if !d.is_empty() => Some(lattice::push::diff_result_to_push_diff(&d)),
                        Ok(_) => None, // empty diff
                        Err(e) => {
                            eprintln!("Warning: could not compute diff, skipping: {}", e);
                            None
                        }
                    }
                });

            let diff_count = push_diff.as_ref().map(|d| d.entries.len());
            let repo_url = lattice::storage::get_git_remote_url();

            match rt.block_on(lattice::push::push(
                &url,
                &key,
                &config.project,
                &nodes,
                &git_sha,
                repo_url,
                push_diff,
            )) {
                Ok(resp) => {
                    if is_json(&format) {
                        let mut result = json!({
                            "success": true,
                            "project_id": resp.project_id,
                            "nodes_upserted": resp.nodes_upserted,
                            "edges_replaced": resp.edges_replaced,
                            "git_sha": git_sha,
                        });
                        if let Some(count) = diff_count {
                            result["diff_entries"] = json!(count);
                        }
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        let mut msg = format!(
                            "✓ Pushed to project #{}: {} nodes, {} edges",
                            resp.project_id, resp.nodes_upserted, resp.edges_replaced
                        );
                        match diff_count {
                            Some(count) => msg.push_str(&format!(" ({} changes)", count)),
                            None => msg.push_str(" (no diff)"),
                        }
                        println!("{}", msg.green());
                    }
                }
                Err(e) => {
                    emit_error(&format, "push_failed", &e.to_string());
                }
            }
        }

        Commands::Diff {
            since,
            since_push,
            api_url,
            api_key,
            md,
            raw,
            format,
        } => {
            let root = get_lattice_root();

            // Resolve --since-push to a SHA
            let effective_since = if since_push {
                let config = load_config(&root);
                let (url, key) = resolve_api_credentials(api_url, api_key, &config, &format);

                let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
                match rt.block_on(lattice::push::fetch_last_push_sha(
                    &url,
                    &key,
                    &config.project,
                )) {
                    Some(sha) => {
                        if !is_json(&format) {
                            eprintln!(
                                "{}",
                                format!(
                                    "Using last push SHA: {}",
                                    &sha[..std::cmp::min(8, sha.len())]
                                )
                                .dimmed()
                            );
                        }
                        Some(sha)
                    }
                    None => {
                        emit_error(
                            &format,
                            "no_prior_push",
                            "No prior push found. Push first with `lattice push`.",
                        );
                    }
                }
            } else {
                since
            };

            if raw {
                match lattice::diff::git_diff_raw(&root, effective_since.as_deref()) {
                    Ok(output) => {
                        if output.is_empty() {
                            println!("No lattice changes detected.");
                        } else {
                            print!("{}", output);
                        }
                    }
                    Err(e) => emit_error(&format, "diff_error", &e.to_string()),
                }
                return;
            }

            match lattice_diff(&root, effective_since.as_deref()) {
                Ok(result) => {
                    if md {
                        println!("{}", format_diff_markdown(&result));
                    } else if is_json(&format) {
                        let to_json_entries = |entries: &[DiffEntry]| -> Vec<serde_json::Value> {
                            entries
                                .iter()
                                .map(|e| {
                                    let mut obj = json!({
                                        "id": e.id,
                                        "title": e.title,
                                        "node_type": format!("{:?}", e.node_type).to_lowercase(),
                                    });
                                    if let Some(ref p) = e.priority {
                                        obj["priority"] = json!(p);
                                    }
                                    if let Some(ref r) = e.resolution {
                                        obj["resolution"] = json!(r);
                                    }
                                    if let Some(ref f) = e.fields {
                                        obj["fields"] = json!(f);
                                    }
                                    obj
                                })
                                .collect()
                        };

                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "base_ref": result.base_ref,
                                "has_changes": !result.is_empty(),
                                "total_changes": result.total_count(),
                                "added": to_json_entries(&result.added),
                                "modified": to_json_entries(&result.modified),
                                "resolved": to_json_entries(&result.resolved),
                                "deleted": to_json_entries(&result.deleted),
                            }))
                            .unwrap()
                        );
                    } else {
                        // Text output
                        if result.is_empty() {
                            println!("{}", "No lattice changes detected.".green());
                            return;
                        }

                        println!(
                            "{}",
                            format!(
                                "Lattice changes since {} ({} total):\n",
                                &result.base_ref[..std::cmp::min(8, result.base_ref.len())],
                                result.total_count()
                            )
                            .bold()
                        );

                        if !result.added.is_empty() {
                            println!("{}", "Added:".green().bold());
                            for entry in &result.added {
                                println!("  {} {}", "+".green(), format_entry_text(entry));
                            }
                            println!();
                        }

                        if !result.modified.is_empty() {
                            println!("{}", "Modified:".yellow().bold());
                            for entry in &result.modified {
                                println!("  {} {}", "~".yellow(), format_entry_text(entry));
                            }
                            println!();
                        }

                        if !result.resolved.is_empty() {
                            println!("{}", "Resolved:".cyan().bold());
                            for entry in &result.resolved {
                                println!("  {} {}", "✓".cyan(), format_entry_text(entry));
                            }
                            println!();
                        }

                        if !result.deleted.is_empty() {
                            println!("{}", "Deleted:".red().bold());
                            for entry in &result.deleted {
                                println!("  {} {}", "-".red(), format_entry_text(entry));
                            }
                            println!();
                        }
                    }
                }
                Err(e) => emit_error(&format, "diff_error", &e.to_string()),
            }
        }

        Commands::Help {
            json,
            compact,
            topic,
        } => {
            if json {
                let catalog = build_command_catalog();
                let output = if compact {
                    compact_catalog(&catalog)
                } else {
                    catalog
                };
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else if let Some(ref t) = topic {
                let catalog = build_command_catalog();
                match t.as_str() {
                    "concepts" => {
                        println!("{}\n", "LATTICE CONCEPTS".bold());
                        let concepts = &catalog["concepts"];

                        // Description
                        if let Some(desc) = catalog["description"].as_str() {
                            println!("{}\n", desc);
                        }

                        // Node types
                        println!("{}\n", "NODE TYPES:".bold());
                        for nt in &["source", "thesis", "requirement", "implementation"] {
                            let node = &concepts["node_types"][nt];
                            let prefix = node["prefix"].as_str().unwrap_or("");
                            let purpose = node["purpose"].as_str().unwrap_or("");
                            let connects = node["connects_to"].as_str().unwrap_or("");
                            println!(
                                "  {} {}",
                                format!("{:<16}", format!("{} ({})", nt, prefix)).cyan(),
                                purpose
                            );
                            println!("  {:<16} {}\n", "", connects.dimmed());
                        }

                        // Edge types
                        println!("{}\n", "EDGE TYPES:".bold());
                        if let Some(edges) = concepts["edges"].as_object() {
                            for (name, desc) in edges {
                                println!("  {:<20} {}", name.cyan(), desc.as_str().unwrap_or(""));
                            }
                        }
                        println!();

                        // Versioning
                        println!("{}\n", "VERSIONING:".bold());
                        if let Some(v) = concepts["versions"].as_str() {
                            println!("  {}\n", v);
                        }

                        // ID conventions
                        println!("{}\n", "ID CONVENTIONS:".bold());
                        if let Some(id) = concepts["id_conventions"].as_str() {
                            println!("  {}", id);
                        }
                    }
                    "workflows" => {
                        println!("{}\n", "LATTICE WORKFLOWS".bold());
                        if let Some(workflows) = catalog["workflows"].as_array() {
                            for wf in workflows {
                                let name = wf["name"].as_str().unwrap_or("");
                                let desc = wf["description"].as_str().unwrap_or("");
                                println!("  {}", name.cyan().bold());
                                println!("  {}\n", desc);
                                if let Some(steps) = wf["steps"].as_array() {
                                    for (i, step) in steps.iter().enumerate() {
                                        println!("    {}. {}", i + 1, step.as_str().unwrap_or(""));
                                    }
                                }
                                println!();
                            }
                        }
                    }
                    "health" => {
                        println!("{}\n", "LATTICE HEALTH".bold());
                        println!(
                            "{} produces a single PASS/WARN/FAIL verdict from three signals:\n",
                            "lattice health".cyan()
                        );

                        println!("  {}", "1. Freshness".bold());
                        println!(
                            "     Time gap between the last code commit and the last .lattice/ commit."
                        );
                        println!(
                            "     WARN when gap exceeds threshold (default 72h, configurable in config.yaml)."
                        );
                        println!("     Cleared by committing any .lattice/ change.\n");

                        println!("  {}", "2. Change Pressure".bold());
                        println!("     Contested theses + version drift in edge bindings.");
                        println!(
                            "     Indicates the graph is under stress and may need a planning cycle.\n"
                        );

                        println!("  {}", "3. Code Impact".bold());
                        println!(
                            "     Files changed since the last .lattice/ commit that are bound in"
                        );
                        println!("     implementation nodes (lattice-tracked files).");
                        println!(
                            "     This is the diff-coupled signal: you changed code the lattice knows"
                        );
                        println!("     about, but haven't updated the lattice to reflect it.\n");

                        println!("{}", "CLEARING A FAIL:".bold());
                        println!(
                            "  1. Re-verify bound implementations: lattice verify/edit the affected nodes"
                        );
                        println!("  2. Stage .lattice/ alongside your code: git add .lattice/");
                        println!(
                            "  3. The gate credits staged .lattice/ changes — no --no-verify needed\n"
                        );

                        println!("{}", "FLAGS:".bold());
                        println!(
                            "  {:<20} Also run lint; lint issues escalate to FAIL",
                            "--strict".cyan()
                        );
                        println!("  {:<20} Exit 2 on FAIL (for CI/hooks)", "--check".cyan());
                        println!("  {:<20} Output as JSON", "--format json".cyan());
                        println!();

                        println!("{}", "CONFIGURATION:".bold());
                        println!(
                            "  freshness_threshold_hours in .lattice/config.yaml (default: 72)"
                        );
                    }
                    other => {
                        eprintln!(
                            "Unknown help topic: {}. Available topics: {}, {}, {}",
                            other.red(),
                            "concepts".cyan(),
                            "workflows".cyan(),
                            "health".cyan()
                        );
                        std::process::exit(1);
                    }
                }
            } else {
                print_grouped_help();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::types::{EdgeReference, Edges};

    // --- Gap 2: Edge summary counting tests ---

    #[test]
    fn test_summarize_edges_empty() {
        let edges = Edges::default();
        assert_eq!(summarize_edges(&edges), None);
    }

    #[test]
    fn test_summarize_edges_single_type() {
        let edges = Edges {
            satisfies: Some(vec![EdgeReference {
                target: "REQ-001".to_string(),
                version: None,
                rationale: None,
            }]),
            ..Default::default()
        };
        assert_eq!(
            summarize_edges(&edges),
            Some("Edges: 1 satisfies".to_string())
        );
    }

    #[test]
    fn test_summarize_edges_multiple_types() {
        let edges = Edges {
            satisfies: Some(vec![
                EdgeReference {
                    target: "REQ-001".to_string(),
                    version: None,
                    rationale: None,
                },
                EdgeReference {
                    target: "REQ-002".to_string(),
                    version: None,
                    rationale: None,
                },
            ]),
            challenges: Some(vec![EdgeReference {
                target: "THX-001".to_string(),
                version: None,
                rationale: None,
            }]),
            ..Default::default()
        };
        let result = summarize_edges(&edges).unwrap();
        assert!(result.contains("2 satisfies"));
        assert!(result.contains("1 challenges"));
    }

    #[test]
    fn test_summarize_edges_some_empty_vec() {
        let edges = Edges {
            satisfies: Some(vec![]),
            ..Default::default()
        }; // Some but empty
        assert_eq!(summarize_edges(&edges), None);
    }

    // --- Gap 4: Command catalog completeness tests ---

    #[test]
    fn test_catalog_contains_all_commands() {
        let catalog = build_command_catalog();
        let commands = catalog["commands"].as_array().unwrap();
        let names: Vec<&str> = commands
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect();

        // All CLI subcommands must be in the catalog
        for expected in &[
            "init",
            "list",
            "get",
            "search",
            "add requirement",
            "add thesis",
            "add source",
            "add implementation",
            "add edge",
            "add message",
            "remove edge",
            "replace edge",
            "resolve",
            "edit",
            "verify",
            "refine",
            "diff",
            "drift",
            "lint",
            "freshness",
            "assess",
            "health",
            "summary",
            "plan",
            "export",
            "update",
            "help",
        ] {
            assert!(
                names.contains(expected),
                "Catalog missing command: {}",
                expected
            );
        }
    }

    #[test]
    fn test_catalog_version_matches_cargo() {
        let catalog = build_command_catalog();
        assert_eq!(
            catalog["version"].as_str().unwrap(),
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn test_catalog_params_have_short_field_where_expected() {
        let catalog = build_command_catalog();
        let commands = catalog["commands"].as_array().unwrap();
        let search = commands.iter().find(|c| c["name"] == "search").unwrap();
        let params = search["parameters"].as_array().unwrap();
        let query_param = params.iter().find(|p| p["name"] == "--query").unwrap();
        assert_eq!(query_param["short"].as_str().unwrap(), "-q");
    }

    #[test]
    fn test_catalog_has_concepts_block() {
        let catalog = build_command_catalog();
        let concepts = &catalog["concepts"];
        assert!(concepts.is_object(), "concepts should be an object");

        // Node types
        let node_types = &concepts["node_types"];
        for nt in &["source", "thesis", "requirement", "implementation"] {
            assert!(node_types[nt].is_object(), "Missing node type: {}", nt);
            assert!(
                node_types[nt]["prefix"].is_string(),
                "Missing prefix for {}",
                nt
            );
            assert!(
                node_types[nt]["purpose"].is_string(),
                "Missing purpose for {}",
                nt
            );
        }

        // Edge types — validate against the canonical EDGE_TYPES constant
        let edges = &concepts["edges"];
        for et in lattice::EDGE_TYPES {
            assert!(edges[*et].is_string(), "Missing edge description: {}", et);
        }

        // Versions and ID conventions
        assert!(concepts["versions"].is_string());
        assert!(concepts["id_conventions"].is_string());
    }

    #[test]
    fn test_catalog_has_workflows() {
        let catalog = build_command_catalog();
        let workflows = catalog["workflows"].as_array().unwrap();
        assert!(
            workflows.len() >= 4,
            "Expected at least 4 workflows, got {}",
            workflows.len()
        );

        let names: Vec<&str> = workflows
            .iter()
            .map(|w| w["name"].as_str().unwrap())
            .collect();
        for expected in &[
            "capture_decision",
            "check_health",
            "respond_to_drift",
            "record_gap",
        ] {
            assert!(names.contains(expected), "Missing workflow: {}", expected);
        }

        // Each workflow has steps
        for w in workflows {
            assert!(
                !w["steps"].as_array().unwrap().is_empty(),
                "Workflow {} has no steps",
                w["name"]
            );
        }
    }

    #[test]
    fn test_catalog_has_structured_examples() {
        let catalog = build_command_catalog();
        let commands = catalog["commands"].as_array().unwrap();
        let init = commands.iter().find(|c| c["name"] == "init").unwrap();
        let examples = init["examples"].as_array().unwrap();
        let first = &examples[0];
        assert!(
            first["command"].is_string(),
            "Examples should have a 'command' field"
        );
        assert!(
            first["explanation"].is_string(),
            "Examples should have an 'explanation' field"
        );
    }

    #[test]
    fn test_catalog_has_related_commands() {
        let catalog = build_command_catalog();
        let commands = catalog["commands"].as_array().unwrap();
        let drift = commands.iter().find(|c| c["name"] == "drift").unwrap();
        assert!(
            drift["related_commands"].is_array(),
            "Commands should have related_commands"
        );
    }

    #[test]
    fn test_catalog_has_output_schema_hints() {
        let catalog = build_command_catalog();
        let commands = catalog["commands"].as_array().unwrap();
        // Commands that produce --format json output should have hints
        for name in &["list", "get", "search", "drift", "summary", "diff"] {
            let cmd = commands.iter().find(|c| c["name"] == *name).unwrap();
            assert!(
                cmd["output_schema_hint"].is_string(),
                "Command '{}' should have output_schema_hint",
                name
            );
        }
    }

    #[test]
    fn test_catalog_has_description() {
        let catalog = build_command_catalog();
        assert!(
            catalog["description"].is_string(),
            "Catalog should have a top-level description"
        );
    }

    #[test]
    fn test_compact_catalog_has_only_signatures() {
        let full = build_command_catalog();
        let compact = compact_catalog(&full);

        // Has version and commands
        assert!(compact["version"].is_string());
        let commands = compact["commands"].as_array().unwrap();
        assert!(!commands.is_empty());

        // Each command has only name and parameters
        for cmd in commands {
            assert!(cmd["name"].is_string());
            assert!(cmd["parameters"].is_array());
            // Should NOT have description, examples, related_commands, output_schema_hint
            assert!(
                cmd["description"].is_null(),
                "compact should omit description"
            );
            assert!(cmd["examples"].is_null(), "compact should omit examples");
            assert!(
                cmd["related_commands"].is_null(),
                "compact should omit related_commands"
            );
            assert!(
                cmd["output_schema_hint"].is_null(),
                "compact should omit output_schema_hint"
            );
        }

        // Should NOT have concepts, workflows, global_flags
        assert!(
            compact["concepts"].is_null(),
            "compact should omit concepts"
        );
        assert!(
            compact["workflows"].is_null(),
            "compact should omit workflows"
        );
    }
}
