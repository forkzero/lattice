//! Lattice CLI - Command-line interface for Lattice operations.
//!
//! Linked requirements: REQ-CLI-001 through REQ-CLI-005, REQ-CORE-009

use clap::{Parser, Subcommand};
use colored::Colorize;
use lattice::{
    AddRequirementOptions, AddSourceOptions, AddThesisOptions, Audience, DriftSeverity,
    ExportOptions, LatticeData, Priority, Status, add_requirement, add_source, add_thesis,
    build_node_index, export_narrative, find_drift, find_lattice_root, load_nodes_by_type,
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
        /// Export format (narrative, json)
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { force: _ } => {
            println!("{}", "lattice init not yet implemented".yellow());
            println!("Would create .lattice/ directory structure");
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
        },

        Commands::List {
            node_type,
            status: _,
            priority: _,
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
                        println!("{} - {}", node.id.cyan(), node.title);
                    }
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
    }
}
