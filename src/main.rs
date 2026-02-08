//! Lattice CLI - Command-line interface for Lattice operations.
//!
//! Linked requirements: REQ-CLI-001 through REQ-CLI-005, REQ-CORE-009

use clap::{Parser, Subcommand};
use colored::Colorize;
use lattice::{
    AddImplementationOptions, AddRequirementOptions, AddSourceOptions, AddThesisOptions, Audience,
    DriftSeverity, ExportOptions, HtmlExportOptions, LatticeData, Plan, Priority, Resolution,
    ResolveOptions, Status, add_implementation, add_requirement, add_source, add_thesis,
    build_node_index, export_html, export_narrative, find_drift, find_lattice_root, generate_plan,
    init_lattice, load_nodes_by_type, resolve_node,
};
use std::env;
use std::process;

#[derive(Parser)]
#[command(name = "lattice")]
#[command(about = "A knowledge coordination protocol for human-agent collaboration")]
#[command(version = "0.1.0")]
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
    },

    /// Get a specific node by ID
    Get {
        /// Node ID
        id: String,
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

    /// Run as MCP server over stdio
    Mcp,
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { force } => {
            let cwd = env::current_dir().expect("Failed to get current directory");

            match init_lattice(&cwd, force) {
                Ok(created) => {
                    for path in &created {
                        let display = path.strip_prefix(&cwd).unwrap_or(path);
                        println!("{}", format!("Created {}", display.display()).green());
                    }
                    println!();
                    println!("{}", "Lattice initialized.".green().bold());
                    println!();
                    println!("Next steps:");
                    println!("  lattice seed              # Bootstrap from vision");
                    println!("  lattice add requirement   # Add a requirement manually");
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
                    Ok(path) => {
                        println!("{}", format!("Created requirement: {}", id).green());
                        println!("{}", format!("File: {}", path.display()).dimmed());
                    }
                    Err(e) => {
                        eprintln!("{}", format!("Error: {}", e).red());
                        process::exit(1);
                    }
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
                    Ok(path) => {
                        println!("{}", format!("Created thesis: {}", id).green());
                        println!("{}", format!("File: {}", path.display()).dimmed());
                    }
                    Err(e) => {
                        eprintln!("{}", format!("Error: {}", e).red());
                        process::exit(1);
                    }
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
                    Ok(path) => {
                        println!("{}", format!("Created source: {}", id).green());
                        println!("{}", format!("File: {}", path.display()).dimmed());
                    }
                    Err(e) => {
                        eprintln!("{}", format!("Error: {}", e).red());
                        process::exit(1);
                    }
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
                    Ok(path) => {
                        println!("{}", format!("Created implementation: {}", id).green());
                        println!("{}", format!("File: {}", path.display()).dimmed());
                    }
                    Err(e) => {
                        eprintln!("{}", format!("Error: {}", e).red());
                        process::exit(1);
                    }
                }
            }
        },

        Commands::List {
            node_type,
            status: _,
            priority: _,
            pending,
        } => {
            let root = get_lattice_root();

            let type_name = match node_type.as_str() {
                "sources" => "sources",
                "theses" => "theses",
                "requirements" => "requirements",
                "implementations" => "implementations",
                _ => {
                    eprintln!("{}", format!("Unknown type: {}", node_type).red());
                    process::exit(1);
                }
            };

            match load_nodes_by_type(&root, type_name) {
                Ok(nodes) => {
                    for node in nodes {
                        // If --pending, only show blocked or deferred items
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
                        } else {
                            // Show resolution status if present
                            if let Some(ref res) = node.resolution {
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
                Err(e) => {
                    eprintln!("{}", format!("Error: {}", e).red());
                    process::exit(1);
                }
            }
        }

        Commands::Resolve {
            id,
            verified,
            blocked,
            deferred,
            wontfix,
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
                eprintln!(
                    "{}",
                    "Must specify one of: --verified, --blocked, --deferred, --wontfix".red()
                );
                process::exit(1);
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
                    println!("{}", format!("Resolved {} as {}", id, status_str).green());
                    if let Some(r) = reason {
                        println!("{}", format!("Reason: {}", r).dimmed());
                    }
                    println!("{}", format!("File: {}", path.display()).dimmed());
                }
                Err(e) => {
                    eprintln!("{}", format!("Error: {}", e).red());
                    process::exit(1);
                }
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

        Commands::Drift { check } => {
            let root = get_lattice_root();

            match find_drift(&root) {
                Ok(reports) => {
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
                        process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("{}", format!("Error: {}", e).red());
                    process::exit(1);
                }
            }
        }

        Commands::Get { id } => {
            let root = get_lattice_root();

            match build_node_index(&root) {
                Ok(index) => {
                    if let Some(node) = index.get(&id) {
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
                    } else {
                        eprintln!("{}", format!("Node not found: {}", id).red());
                        process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("{}", format!("Error: {}", e).red());
                    process::exit(1);
                }
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

        Commands::Mcp => {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            if let Err(e) = rt.block_on(lattice::mcp::run_server()) {
                eprintln!("{}", format!("MCP server error: {}", e).red());
                process::exit(1);
            }
        }
    }
}
