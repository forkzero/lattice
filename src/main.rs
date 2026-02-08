//! Lattice CLI - Command-line interface for Lattice operations.
//!
//! Linked requirements: REQ-CLI-001 through REQ-CLI-005, REQ-CORE-009

use clap::{Parser, Subcommand};
use colored::Colorize;
use lattice::{
    AddImplementationOptions, AddRequirementOptions, AddSourceOptions, AddThesisOptions, Audience,
    DriftSeverity, ExportOptions, GapType, HtmlExportOptions, LatticeData, LintSeverity, Plan,
    Priority, RefineOptions, Resolution, ResolveOptions, Status, VerifyOptions, add_implementation,
    add_requirement, add_source, add_thesis, build_node_index, export_html, export_narrative,
    find_drift, find_lattice_root, fix_issues, generate_plan, init_lattice, lint_lattice,
    load_nodes_by_type, refine_requirement, resolve_node, verify_implementation,
};
use serde_json::json;
use std::collections::HashSet;
use std::env;
use std::process;

#[derive(Parser)]
#[command(name = "lattice")]
#[command(about = "A knowledge coordination protocol for human-agent collaboration")]
#[command(version = "0.1.0")]
#[command(disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new lattice in the current directory
    Init {
        /// Overwrite existing lattice
        #[arg(short, long)]
        force: bool,

        /// Also install agent definitions (.claude/agents/)
        #[arg(long)]
        agents: bool,
    },

    /// Add a node to the lattice
    Add {
        #[command(subcommand)]
        add_command: AddCommands,
    },

    /// List nodes of a given type
    List {
        /// Node type (sources, theses, requirements, implementations)
        node_type: String,

        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by priority (requirements only)
        #[arg(short, long)]
        priority: Option<String>,

        /// Show only pending items (blocked or deferred)
        #[arg(long)]
        pending: bool,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Resolve a requirement with a status
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

    /// Plan implementation of requirements
    Plan {
        /// Requirement IDs to plan (e.g., REQ-CLI-001 REQ-CLI-002)
        #[arg(required = true)]
        requirements: Vec<String>,
    },

    /// Check for version drift in the lattice
    Drift {
        /// Exit with non-zero status if drift detected
        #[arg(long)]
        check: bool,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Get a specific node by ID
    Get {
        /// Node ID
        id: String,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Export the lattice to various formats
    Export {
        /// Export format (narrative, json, html)
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

    /// Show a compact status summary of the lattice
    Summary {
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Lint lattice files for issues
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

    /// Verify that an implementation satisfies a requirement
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

    /// Refine a requirement by creating a sub-requirement from a discovered gap
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
        #[arg(long)]
        proposed: Option<String>,

        /// Implementation ID that discovered this gap
        #[arg(long)]
        implementation: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Search nodes with filters (text, priority, resolution, tags, category, proximity)
    Search {
        /// Node type to search (sources, theses, requirements, implementations)
        #[arg(short = 't', long, default_value = "requirements")]
        node_type: String,

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

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Run as MCP server over stdio
    Mcp,

    /// Output CLAUDE.md integration snippet
    Prompt {
        /// Output MCP version instead of CLI version
        #[arg(long)]
        mcp: bool,
    },

    /// Show available commands (use --json for machine-readable catalog)
    Help {
        /// Output as structured JSON for agent consumption
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum AddCommands {
    /// Add a requirement to the lattice
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

    /// Add a thesis to the lattice
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

    /// Add a source to the lattice
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

    /// Add an implementation to the lattice
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

fn split_csv(s: Option<String>) -> Option<Vec<String>> {
    s.map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
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

fn build_command_catalog() -> serde_json::Value {
    let param = |name: &str, typ: &str, required: bool, desc: &str| -> serde_json::Value {
        json!({
            "name": name,
            "type": typ,
            "required": required,
            "description": desc
        })
    };

    json!({
        "version": "0.1.0",
        "commands": [
            {
                "name": "init",
                "description": "Initialize a new lattice in the current directory",
                "parameters": [
                    param("--force", "bool", false, "Overwrite existing lattice"),
                    param("--agents", "bool", false, "Install agent definitions (.claude/agents/)")
                ],
                "examples": [
                    "lattice init",
                    "lattice init --force",
                    "lattice init --agents"
                ]
            },
            {
                "name": "list",
                "description": "List nodes of a given type",
                "parameters": [
                    param("node_type", "string", true, "Node type: sources, theses, requirements, implementations"),
                    param("--status", "string", false, "Filter by status"),
                    param("--priority", "string", false, "Filter by priority (P0, P1, P2)"),
                    param("--pending", "bool", false, "Show only blocked/deferred items"),
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice list requirements",
                    "lattice list requirements --priority P0 --format json",
                    "lattice list theses"
                ]
            },
            {
                "name": "get",
                "description": "Get a specific node by ID with full details",
                "parameters": [
                    param("id", "string", true, "Node ID (e.g. REQ-CORE-001)"),
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice get REQ-CORE-001",
                    "lattice get THX-AGENT-NATIVE-TOOLS --format json"
                ]
            },
            {
                "name": "search",
                "description": "Search nodes with filters (text, priority, resolution, tags, category, graph proximity)",
                "parameters": [
                    param("--node-type", "string", false, "Node type to search (default: requirements)"),
                    param("--query", "string", false, "Text search in title and body"),
                    param("--priority", "string", false, "Filter by priority (P0, P1, P2)"),
                    param("--resolution", "string", false, "Filter: verified, blocked, deferred, wontfix, unresolved"),
                    param("--tag", "string", false, "Filter by single tag"),
                    param("--tags", "string", false, "Comma-separated tags (all must match)"),
                    param("--category", "string", false, "Filter by category"),
                    param("--id-prefix", "string", false, "Filter by ID prefix"),
                    param("--related-to", "string", false, "Find nodes related to this node ID"),
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice search --query 'product owner' --format json",
                    "lattice search --priority P0 --resolution unresolved",
                    "lattice search --tag agent --category AGENT"
                ]
            },
            {
                "name": "add requirement",
                "description": "Add a requirement node to the lattice",
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
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice add requirement --id REQ-FEAT-001 --title 'New feature' --body 'Description' --priority P1 --category FEAT"
                ]
            },
            {
                "name": "add thesis",
                "description": "Add a thesis node to the lattice",
                "parameters": [
                    param("--id", "string", true, "Thesis ID (e.g. THX-AGENT-PROTOCOL)"),
                    param("--title", "string", true, "Thesis title"),
                    param("--body", "string", true, "Thesis body/description"),
                    param("--category", "string", true, "Category: value_prop, market, technical, risk, competitive"),
                    param("--confidence", "float", false, "Confidence level 0.0-1.0"),
                    param("--tags", "string", false, "Comma-separated tags"),
                    param("--supported-by", "string", false, "Comma-separated source IDs"),
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice add thesis --id THX-NEW --title 'Thesis' --body 'Claim' --category technical"
                ]
            },
            {
                "name": "add source",
                "description": "Add a source node to the lattice",
                "parameters": [
                    param("--id", "string", true, "Source ID (e.g. SRC-PAPER-001)"),
                    param("--title", "string", true, "Source title"),
                    param("--body", "string", true, "Source body/summary"),
                    param("--url", "string", false, "Source URL"),
                    param("--source-type", "string", false, "Type: paper, article, report, data"),
                    param("--tags", "string", false, "Comma-separated tags"),
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice add source --id SRC-NEW --title 'Paper' --body 'Summary' --url https://example.com"
                ]
            },
            {
                "name": "add implementation",
                "description": "Add an implementation node to the lattice",
                "parameters": [
                    param("--id", "string", true, "Implementation ID (e.g. IMP-STORAGE-001)"),
                    param("--title", "string", true, "Implementation title"),
                    param("--body", "string", true, "Implementation body/description"),
                    param("--satisfies", "string", false, "Comma-separated requirement IDs"),
                    param("--files", "string", false, "Comma-separated file paths"),
                    param("--tags", "string", false, "Comma-separated tags"),
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice add implementation --id IMP-NEW --title 'Impl' --body 'Description' --satisfies REQ-CORE-001"
                ]
            },
            {
                "name": "resolve",
                "description": "Resolve a requirement with a status",
                "parameters": [
                    param("id", "string", true, "Requirement ID"),
                    param("--verified", "bool", false, "Mark as verified"),
                    param("--blocked", "string", false, "Mark as blocked with reason"),
                    param("--deferred", "string", false, "Mark as deferred with reason"),
                    param("--wontfix", "string", false, "Mark as wontfix with reason"),
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice resolve REQ-CORE-001 --verified",
                    "lattice resolve REQ-API-002 --deferred 'Post-MVP'"
                ]
            },
            {
                "name": "verify",
                "description": "Record that an implementation satisfies a requirement",
                "parameters": [
                    param("implementation", "string", true, "Implementation ID"),
                    param("relation", "string", true, "Must be 'satisfies'"),
                    param("requirement", "string", true, "Requirement ID"),
                    param("--tests-pass", "bool", false, "Record that tests pass"),
                    param("--coverage", "float", false, "Coverage percentage 0.0-1.0"),
                    param("--files", "string", false, "Comma-separated evidence file paths"),
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice verify IMP-STORAGE-001 satisfies REQ-CORE-004 --tests-pass --coverage 0.94"
                ]
            },
            {
                "name": "refine",
                "description": "Create a sub-requirement from a discovered gap in an existing requirement",
                "parameters": [
                    param("parent", "string", true, "Parent requirement ID"),
                    param("--gap-type", "string", true, "Gap type: clarification, design_decision, missing_requirement, contradiction"),
                    param("--title", "string", true, "Brief title for the sub-requirement"),
                    param("--description", "string", true, "What is underspecified and why"),
                    param("--proposed", "string", false, "Proposed resolution"),
                    param("--implementation", "string", false, "Implementation ID that discovered this gap"),
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice refine REQ-CORE-005 --gap-type design_decision --title 'Drift threshold' --description 'Should minor version drift be flagged?'"
                ]
            },
            {
                "name": "drift",
                "description": "Check for version drift in edge bindings",
                "parameters": [
                    param("--check", "bool", false, "Exit with code 2 if drift detected"),
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice drift",
                    "lattice drift --check --format json"
                ]
            },
            {
                "name": "lint",
                "description": "Lint lattice files for structural issues",
                "parameters": [
                    param("--fix", "bool", false, "Attempt to auto-fix fixable issues"),
                    param("--strict", "bool", false, "Exit with non-zero status on any issue"),
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice lint",
                    "lattice lint --fix",
                    "lattice lint --strict --format json"
                ]
            },
            {
                "name": "summary",
                "description": "Show a compact status overview of the lattice",
                "parameters": [
                    param("--format", "string", false, "Output format: text, json (default: text)")
                ],
                "examples": [
                    "lattice summary",
                    "lattice summary --format json"
                ]
            },
            {
                "name": "plan",
                "description": "Plan implementation order for requirements based on dependency graph",
                "parameters": [
                    param("requirements", "string[]", true, "Requirement IDs to plan")
                ],
                "examples": [
                    "lattice plan REQ-CLI-001 REQ-CLI-002"
                ]
            },
            {
                "name": "export",
                "description": "Export the lattice to narrative, JSON, or HTML",
                "parameters": [
                    param("--format", "string", false, "Export format: narrative, json, html (default: narrative)"),
                    param("--audience", "string", false, "Target audience: investor, contributor, overview (default: overview)"),
                    param("--title", "string", false, "Document title (default: Lattice)"),
                    param("--include-internal", "bool", false, "Include nodes marked as internal"),
                    param("--output", "string", false, "Output directory for HTML export")
                ],
                "examples": [
                    "lattice export",
                    "lattice export --format json",
                    "lattice export --format html --output ./docs"
                ]
            },
            {
                "name": "help",
                "description": "Show available commands (use --json for machine-readable catalog)",
                "parameters": [
                    param("--json", "bool", false, "Output as structured JSON for agent consumption")
                ],
                "examples": [
                    "lattice help",
                    "lattice help --json"
                ]
            }
        ],
        "exit_codes": {
            "0": "Success",
            "1": "Error",
            "2": "Actionable condition (drift detected, lint issues)"
        },
        "global_flags": {
            "--format json": "Available on all read and write commands for structured output",
            "--help": "Show help for any command",
            "--version": "Show version"
        }
    })
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { force, agents } => {
            let cwd = env::current_dir().expect("Failed to get current directory");

            match init_lattice(&cwd, force) {
                Ok(created) => {
                    for path in &created {
                        let display = path.strip_prefix(&cwd).unwrap_or(path);
                        println!("{}", format!("Created {}", display.display()).green());
                    }

                    if agents {
                        match install_agent_definitions(&cwd) {
                            Ok(agent_paths) => {
                                for path in &agent_paths {
                                    let display = path.strip_prefix(&cwd).unwrap_or(path);
                                    println!(
                                        "{}",
                                        format!("Created {}", display.display()).green()
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "{}",
                                    format!("Warning: failed to install agents: {}", e).yellow()
                                );
                            }
                        }
                    }

                    println!();
                    println!("{}", "Lattice initialized.".green().bold());
                    println!();
                    println!("Next steps:");
                    println!("  lattice seed              # Bootstrap from vision");
                    println!("  lattice add requirement   # Add a requirement manually");
                    if !agents {
                        println!("  lattice init --agents     # Install agent definitions");
                    }
                }
                Err(e) => {
                    eprintln!("{}", format!("Error: {}", e).red());
                    process::exit(1);
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
        },

        Commands::List {
            node_type,
            status: _,
            priority: _,
            pending,
            format,
        } => {
            let root = get_lattice_root();

            let type_name = match node_type.as_str() {
                "sources" | "theses" | "requirements" | "implementations" => node_type.as_str(),
                _ => emit_error(
                    &format,
                    "unknown_type",
                    &format!("Unknown type: {}", node_type),
                ),
            };

            match load_nodes_by_type(&root, type_name) {
                Ok(nodes) => {
                    if is_json(&format) {
                        let filtered: Vec<_> = if pending {
                            nodes
                                .into_iter()
                                .filter(|n| {
                                    n.resolution.as_ref().is_some_and(|r| {
                                        matches!(
                                            r.status,
                                            Resolution::Blocked | Resolution::Deferred
                                        )
                                    })
                                })
                                .collect()
                        } else {
                            nodes
                        };
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
                        for node in nodes {
                            if pending {
                                if let Some(ref res) = node.resolution {
                                    match res.status {
                                        Resolution::Blocked | Resolution::Deferred => {
                                            let status_str =
                                                format!("[{:?}]", res.status).to_lowercase();
                                            let reason = res.reason.as_deref().unwrap_or("");
                                            println!(
                                                "{} {} - {} {}",
                                                node.id.cyan(),
                                                status_str.yellow(),
                                                node.title,
                                                reason.dimmed()
                                            );
                                        }
                                        _ => {}
                                    }
                                }
                            } else if let Some(ref res) = node.resolution {
                                let status_str = format!("[{:?}]", res.status).to_lowercase();
                                println!(
                                    "{} {} - {}",
                                    node.id.cyan(),
                                    status_str.yellow(),
                                    node.title
                                );
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

        Commands::Drift { check, format } => {
            let root = get_lattice_root();

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
                        let nodes: Vec<_> = index.values().collect();
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&nodes)
                                .unwrap_or_else(|_| "[]".to_string())
                        );
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

                let data = LatticeData {
                    sources,
                    theses,
                    requirements,
                    implementations,
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

                let data = LatticeData {
                    sources,
                    theses,
                    requirements,
                    implementations,
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
                        "implementations": implementations.len()
                    },
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
                    "  {} sources, {} theses, {} requirements, {} implementations",
                    sources.len(),
                    theses.len(),
                    requirements.len(),
                    implementations.len()
                );
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
            node_type,
            query,
            priority,
            resolution,
            tag,
            tags,
            category,
            id_prefix,
            related_to,
            format,
        } => {
            let type_name = match node_type.as_str() {
                "sources" => "sources",
                "theses" => "theses",
                "requirements" => "requirements",
                "implementations" => "implementations",
                _ => {
                    emit_error(
                        &format,
                        "invalid_type",
                        &format!(
                            "Unknown type: {}. Use: sources, theses, requirements, implementations",
                            node_type
                        ),
                    );
                }
            };

            let root = get_lattice_root();

            let nodes = match load_nodes_by_type(&root, type_name) {
                Ok(n) => n,
                Err(e) => emit_error(&format, "load_error", &e.to_string()),
            };

            // Build graph proximity set if related_to is specified
            let related_ids: Option<HashSet<String>> = if let Some(ref related_to_id) = related_to {
                match build_node_index(&root) {
                    Ok(index) => {
                        if let Some(source_node) = index.get(related_to_id) {
                            let mut related = HashSet::new();
                            let mut source_targets = HashSet::new();

                            if let Some(edges) = &source_node.edges {
                                for el in [
                                    &edges.derives_from,
                                    &edges.depends_on,
                                    &edges.satisfies,
                                    &edges.supported_by,
                                ]
                                .into_iter()
                                .flatten()
                                {
                                    for edge in el {
                                        source_targets.insert(edge.target.clone());
                                    }
                                }
                            }

                            // Find nodes sharing edge targets
                            for node in index.values() {
                                if node.id == *related_to_id {
                                    continue;
                                }
                                if let Some(edges) = &node.edges {
                                    let mut node_targets = HashSet::new();
                                    for el in [
                                        &edges.derives_from,
                                        &edges.depends_on,
                                        &edges.satisfies,
                                        &edges.supported_by,
                                    ]
                                    .into_iter()
                                    .flatten()
                                    {
                                        for edge in el {
                                            node_targets.insert(edge.target.clone());
                                        }
                                    }
                                    if !source_targets.is_disjoint(&node_targets) {
                                        related.insert(node.id.clone());
                                    }
                                }
                            }

                            // Include direct references
                            for target in &source_targets {
                                related.insert(target.clone());
                            }

                            // Include nodes referencing the source
                            for node in index.values() {
                                if let Some(edges) = &node.edges {
                                    let targets_source = [
                                        &edges.derives_from,
                                        &edges.depends_on,
                                        &edges.satisfies,
                                        &edges.supported_by,
                                    ]
                                    .iter()
                                    .any(|edge_list| {
                                        edge_list.as_ref().is_some_and(|v| {
                                            v.iter().any(|e| e.target == *related_to_id)
                                        })
                                    });
                                    if targets_source {
                                        related.insert(node.id.clone());
                                    }
                                }
                            }

                            Some(related)
                        } else {
                            emit_error(
                                &format,
                                "node_not_found",
                                &format!("Node not found: {}", related_to_id),
                            );
                        }
                    }
                    Err(e) => emit_error(&format, "index_error", &e.to_string()),
                }
            } else {
                None
            };

            let tags_vec: Option<Vec<String>> =
                tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

            let results: Vec<_> = nodes
                .iter()
                .filter(|n| {
                    if let Some(ref prefix) = id_prefix
                        && !n.id.to_uppercase().starts_with(&prefix.to_uppercase())
                    {
                        return false;
                    }
                    if let Some(ref related) = related_ids
                        && !related.contains(&n.id)
                    {
                        return false;
                    }
                    if let Some(ref q) = query {
                        let q_lower = q.to_lowercase();
                        if !n.title.to_lowercase().contains(&q_lower)
                            && !n.body.to_lowercase().contains(&q_lower)
                        {
                            return false;
                        }
                    }
                    if let Some(ref p) = priority {
                        let node_priority = n.priority.as_ref().map(|p| format!("{:?}", p));
                        if node_priority.as_deref() != Some(p.to_uppercase().as_str()) {
                            return false;
                        }
                    }
                    if let Some(ref res) = resolution {
                        let res_lower = res.to_lowercase();
                        let matches = match res_lower.as_str() {
                            "verified" => matches!(
                                n.resolution.as_ref().map(|r| &r.status),
                                Some(Resolution::Verified)
                            ),
                            "blocked" => matches!(
                                n.resolution.as_ref().map(|r| &r.status),
                                Some(Resolution::Blocked)
                            ),
                            "deferred" => matches!(
                                n.resolution.as_ref().map(|r| &r.status),
                                Some(Resolution::Deferred)
                            ),
                            "wontfix" => matches!(
                                n.resolution.as_ref().map(|r| &r.status),
                                Some(Resolution::Wontfix)
                            ),
                            "unresolved" | "open" => n.resolution.is_none(),
                            _ => true,
                        };
                        if !matches {
                            return false;
                        }
                    }
                    if let Some(ref t) = tag {
                        let tag_lower = t.to_lowercase();
                        let has_tag = n
                            .tags
                            .as_ref()
                            .map(|tags| tags.iter().any(|t| t.to_lowercase() == tag_lower))
                            .unwrap_or(false);
                        if !has_tag {
                            return false;
                        }
                    }
                    if let Some(ref search_tags) = tags_vec {
                        let node_tags: HashSet<String> = n
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
                    if let Some(ref cat) = category {
                        let matches_cat = n
                            .category
                            .as_ref()
                            .map(|c| c.to_lowercase() == cat.to_lowercase())
                            .unwrap_or(false);
                        if !matches_cat {
                            return false;
                        }
                    }
                    true
                })
                .collect();

            if is_json(&format) {
                let json_results: Vec<_> = results
                    .iter()
                    .map(|n| {
                        json!({
                            "id": n.id,
                            "title": n.title,
                            "body": n.body,
                            "version": n.version,
                            "priority": n.priority.as_ref().map(|p| format!("{:?}", p)),
                            "resolution": n.resolution.as_ref().map(|r| format!("{:?}", r.status).to_lowercase()),
                            "category": n.category,
                            "tags": n.tags
                        })
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
                println!("{}", format!("Found {} results:", results.len()).bold());
                for n in &results {
                    let priority_str = n
                        .priority
                        .as_ref()
                        .map(|p| format!("[{:?}]", p))
                        .unwrap_or_default();
                    let resolution_str = n
                        .resolution
                        .as_ref()
                        .map(|r| format!(" ({})", format!("{:?}", r.status).to_lowercase()))
                        .unwrap_or_default();
                    println!(
                        "  {} {} {}{}",
                        n.id.cyan(),
                        priority_str.yellow(),
                        n.title,
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

        Commands::Prompt { mcp } => {
            if mcp {
                print!("{}", include_str!("../prompts/LATTICE_MCP_CLAUDE_MD.md"));
            } else {
                print!("{}", include_str!("../prompts/LATTICE_CLAUDE_MD.md"));
            }
        }

        Commands::Help { json } => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&build_command_catalog()).unwrap()
                );
            } else {
                // Human-readable help
                let catalog = build_command_catalog();
                println!(
                    "{}\n",
                    "LATTICE - A knowledge coordination protocol for human-agent collaboration"
                        .bold()
                );
                println!("{}", "COMMANDS:".bold());
                if let Some(commands) = catalog["commands"].as_array() {
                    for cmd in commands {
                        let name = cmd["name"].as_str().unwrap_or("");
                        let desc = cmd["description"].as_str().unwrap_or("");
                        println!("  {:<22} {}", name.cyan(), desc);
                    }
                }
                println!();
                println!("{}", "EXIT CODES:".bold());
                println!("  {}  Success", "0".cyan());
                println!("  {}  Error", "1".cyan());
                println!(
                    "  {}  Actionable condition (drift detected, lint issues)",
                    "2".cyan()
                );
                println!();
                println!(
                    "Use {} for machine-readable command catalog.",
                    "lattice help --json".cyan()
                );
                println!(
                    "Use {} for help on a specific command.",
                    "lattice <command> --help".cyan()
                );
            }
        }
    }
}
