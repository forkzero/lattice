//! HTML export for Lattice.
//!
//! Generates a static HTML documentation site from the lattice data.

use crate::export::LatticeData;
use crate::types::{LatticeNode, NodeMeta, Priority, Resolution};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use tera::{Context, Tera};

const BASE_TEMPLATE: &str = include_str!("templates/base.html");
const DASHBOARD_TEMPLATE: &str = include_str!("templates/dashboard.html");

/// Options for HTML export.
pub struct HtmlExportOptions {
    pub output_dir: PathBuf,
    pub title: String,
}

/// Statistics computed from the lattice data.
#[derive(Debug)]
pub struct Statistics {
    pub sources: usize,
    pub theses: usize,
    pub requirements: usize,
    pub implementations: usize,
    pub implemented: usize,
    pub coverage_pct: usize,
    pub verified: usize,
    pub blocked: usize,
    pub deferred: usize,
    pub unresolved: usize,
    pub wontfix: usize,
    pub p0: usize,
    pub p1: usize,
    pub p2: usize,
    pub p0_verified: usize,
    pub p1_verified: usize,
    pub p2_verified: usize,
}

/// A thesis node in the traceability tree.
#[derive(Debug)]
pub struct TraceabilityThesis {
    pub id: String,
    pub title: String,
    pub requirements: Vec<TraceabilityRequirement>,
}

/// A requirement node in the traceability tree.
#[derive(Debug)]
pub struct TraceabilityRequirement {
    pub id: String,
    pub title: String,
    pub priority: Option<String>,
    pub resolution: Option<String>,
    pub implementations: Vec<TraceabilityImplementation>,
}

/// An implementation node in the traceability tree.
#[derive(Debug)]
pub struct TraceabilityImplementation {
    pub id: String,
    pub title: String,
}

/// Compute statistics from the lattice data.
pub fn compute_statistics(data: &LatticeData) -> Statistics {
    let mut implemented_ids: HashSet<String> = HashSet::new();
    for impl_node in &data.implementations {
        if let Some(satisfies) = impl_node.edges.as_ref().and_then(|e| e.satisfies.as_ref()) {
            for edge in satisfies {
                implemented_ids.insert(edge.target.clone());
            }
        }
    }

    let implemented = data
        .requirements
        .iter()
        .filter(|r| implemented_ids.contains(&r.id))
        .count();

    let total_reqs = data.requirements.len();
    let coverage_pct = if total_reqs > 0 {
        (implemented * 100) / total_reqs
    } else {
        0
    };

    let mut verified = 0;
    let mut blocked = 0;
    let mut deferred = 0;
    let mut unresolved = 0;
    let mut wontfix = 0;
    let mut p0 = 0;
    let mut p1 = 0;
    let mut p2 = 0;
    let mut p0_verified = 0;
    let mut p1_verified = 0;
    let mut p2_verified = 0;

    for req in &data.requirements {
        let is_verified = req
            .resolution
            .as_ref()
            .map(|r| r.status == Resolution::Verified)
            .unwrap_or(false);

        match req.resolution.as_ref().map(|r| &r.status) {
            Some(Resolution::Verified) => verified += 1,
            Some(Resolution::Blocked) => blocked += 1,
            Some(Resolution::Deferred) => deferred += 1,
            Some(Resolution::Wontfix) => wontfix += 1,
            None => unresolved += 1,
        }

        match req.priority {
            Some(Priority::P0) => {
                p0 += 1;
                if is_verified {
                    p0_verified += 1;
                }
            }
            Some(Priority::P1) => {
                p1 += 1;
                if is_verified {
                    p1_verified += 1;
                }
            }
            Some(Priority::P2) => {
                p2 += 1;
                if is_verified {
                    p2_verified += 1;
                }
            }
            None => {}
        }
    }

    Statistics {
        sources: data.sources.len(),
        theses: data.theses.len(),
        requirements: data.requirements.len(),
        implementations: data.implementations.len(),
        implemented,
        coverage_pct,
        verified,
        blocked,
        deferred,
        unresolved,
        wontfix,
        p0,
        p1,
        p2,
        p0_verified,
        p1_verified,
        p2_verified,
    }
}

/// Build a traceability tree from theses -> requirements -> implementations.
pub fn build_traceability_tree(data: &LatticeData) -> Vec<TraceabilityThesis> {
    // Build reverse index: requirement ID -> implementations that satisfy it
    let mut req_to_impls: HashMap<&str, Vec<&LatticeNode>> = HashMap::new();
    for impl_node in &data.implementations {
        if let Some(satisfies) = impl_node.edges.as_ref().and_then(|e| e.satisfies.as_ref()) {
            for edge in satisfies {
                req_to_impls
                    .entry(edge.target.as_str())
                    .or_default()
                    .push(impl_node);
            }
        }
    }

    // Build reverse index: thesis ID -> requirements that derive from it
    let mut thesis_to_reqs: HashMap<&str, Vec<&LatticeNode>> = HashMap::new();
    for req in &data.requirements {
        if let Some(derives_from) = req.edges.as_ref().and_then(|e| e.derives_from.as_ref()) {
            for edge in derives_from {
                thesis_to_reqs
                    .entry(edge.target.as_str())
                    .or_default()
                    .push(req);
            }
        }
    }

    // Build the tree
    let mut tree = Vec::new();
    for thesis in &data.theses {
        let requirements: Vec<TraceabilityRequirement> = thesis_to_reqs
            .get(thesis.id.as_str())
            .map(|reqs| {
                reqs.iter()
                    .map(|req| {
                        let implementations: Vec<TraceabilityImplementation> = req_to_impls
                            .get(req.id.as_str())
                            .map(|impls| {
                                impls
                                    .iter()
                                    .map(|i| TraceabilityImplementation {
                                        id: i.id.clone(),
                                        title: i.title.clone(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();

                        TraceabilityRequirement {
                            id: req.id.clone(),
                            title: req.title.clone(),
                            priority: req.priority.as_ref().map(|p| format!("{:?}", p)),
                            resolution: req
                                .resolution
                                .as_ref()
                                .map(|r| format!("{:?}", r.status).to_lowercase()),
                            implementations,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        tree.push(TraceabilityThesis {
            id: thesis.id.clone(),
            title: thesis.title.clone(),
            requirements,
        });
    }

    tree
}

/// Simplified node representation for templates.
#[derive(Debug, serde::Serialize)]
struct TemplateNode {
    id: String,
    title: String,
    body: String,
    priority: Option<String>,
    resolution: Option<String>,
    url: Option<String>,
    citations: Vec<String>,
}

impl From<&LatticeNode> for TemplateNode {
    fn from(node: &LatticeNode) -> Self {
        let (url, citations) = node
            .meta
            .as_ref()
            .map(|m| {
                if let NodeMeta::Source(sm) = m {
                    (sm.url.clone(), sm.citations.clone().unwrap_or_default())
                } else {
                    (None, Vec::new())
                }
            })
            .unwrap_or((None, Vec::new()));

        TemplateNode {
            id: node.id.clone(),
            title: node.title.clone(),
            body: node.body.clone(),
            priority: node.priority.as_ref().map(|p| format!("{:?}", p)),
            resolution: node
                .resolution
                .as_ref()
                .map(|r| format!("{:?}", r.status).to_lowercase()),
            url,
            citations,
        }
    }
}

/// Simplified traceability structures for templates.
#[derive(Debug, serde::Serialize)]
struct TemplateTraceabilityThesis {
    id: String,
    title: String,
    requirements: Vec<TemplateTraceabilityRequirement>,
}

#[derive(Debug, serde::Serialize)]
struct TemplateTraceabilityRequirement {
    id: String,
    title: String,
    priority: Option<String>,
    resolution: Option<String>,
    implementations: Vec<TemplateTraceabilityImplementation>,
}

#[derive(Debug, serde::Serialize)]
struct TemplateTraceabilityImplementation {
    id: String,
    title: String,
}

/// Build the Tera context from lattice data.
fn build_context(data: &LatticeData, options: &HtmlExportOptions) -> Context {
    let stats = compute_statistics(data);
    let traceability = build_traceability_tree(data);

    let mut context = Context::new();
    context.insert("title", &options.title);
    context.insert(
        "generated_at",
        &chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
    );

    // Statistics
    let stats_map: HashMap<&str, usize> = [
        ("sources", stats.sources),
        ("theses", stats.theses),
        ("requirements", stats.requirements),
        ("implementations", stats.implementations),
        ("implemented", stats.implemented),
        ("coverage_pct", stats.coverage_pct),
        ("verified", stats.verified),
        ("blocked", stats.blocked),
        ("deferred", stats.deferred),
        ("unresolved", stats.unresolved),
        ("wontfix", stats.wontfix),
        ("p0", stats.p0),
        ("p1", stats.p1),
        ("p2", stats.p2),
        ("p0_verified", stats.p0_verified),
        ("p1_verified", stats.p1_verified),
        ("p2_verified", stats.p2_verified),
    ]
    .into_iter()
    .collect();
    context.insert("stats", &stats_map);

    // Traceability tree
    let template_traceability: Vec<TemplateTraceabilityThesis> = traceability
        .into_iter()
        .map(|t| TemplateTraceabilityThesis {
            id: t.id,
            title: t.title,
            requirements: t
                .requirements
                .into_iter()
                .map(|r| TemplateTraceabilityRequirement {
                    id: r.id,
                    title: r.title,
                    priority: r.priority,
                    resolution: r.resolution,
                    implementations: r
                        .implementations
                        .into_iter()
                        .map(|i| TemplateTraceabilityImplementation {
                            id: i.id,
                            title: i.title,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();
    context.insert("traceability", &template_traceability);

    // Node lists
    let sources: Vec<TemplateNode> = data.sources.iter().map(TemplateNode::from).collect();
    let theses: Vec<TemplateNode> = data.theses.iter().map(TemplateNode::from).collect();
    let requirements: Vec<TemplateNode> =
        data.requirements.iter().map(TemplateNode::from).collect();
    let implementations: Vec<TemplateNode> = data
        .implementations
        .iter()
        .map(TemplateNode::from)
        .collect();

    context.insert("sources", &sources);
    context.insert("theses", &theses);
    context.insert("requirements", &requirements);
    context.insert("implementations", &implementations);

    context
}

/// Export the lattice to HTML.
///
/// Returns the path to the generated index.html file.
pub fn export_html(data: &LatticeData, options: &HtmlExportOptions) -> Result<PathBuf, String> {
    // Create output directory
    fs::create_dir_all(&options.output_dir)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // Initialize Tera with embedded templates
    let mut tera = Tera::default();
    tera.add_raw_template("base.html", BASE_TEMPLATE)
        .map_err(|e| format!("Failed to add base template: {}", e))?;
    tera.add_raw_template("dashboard.html", DASHBOARD_TEMPLATE)
        .map_err(|e| format!("Failed to add dashboard template: {}", e))?;

    // Build context
    let context = build_context(data, options);

    // Render template
    let html = tera
        .render("dashboard.html", &context)
        .map_err(|e| format!("Failed to render template: {}", e))?;

    // Write output
    let output_path = options.output_dir.join("index.html");
    fs::write(&output_path, html).map_err(|e| format!("Failed to write output: {}", e))?;

    Ok(output_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NodeType, ResolutionInfo, Status};

    fn create_test_data() -> LatticeData {
        LatticeData {
            sources: vec![LatticeNode {
                id: "SRC-001".to_string(),
                node_type: NodeType::Source,
                title: "Test Source".to_string(),
                body: "A test source".to_string(),
                status: Status::Active,
                version: "1.0.0".to_string(),
                created_at: "2024-01-01".to_string(),
                created_by: "test".to_string(),
                requested_by: None,
                priority: None,
                category: None,
                tags: None,
                acceptance: None,
                visibility: None,
                resolution: None,
                meta: None,
                edges: None,
            }],
            theses: vec![LatticeNode {
                id: "THX-001".to_string(),
                node_type: NodeType::Thesis,
                title: "Test Thesis".to_string(),
                body: "A test thesis".to_string(),
                status: Status::Active,
                version: "1.0.0".to_string(),
                created_at: "2024-01-01".to_string(),
                created_by: "test".to_string(),
                requested_by: None,
                priority: None,
                category: None,
                tags: None,
                acceptance: None,
                visibility: None,
                resolution: None,
                meta: None,
                edges: None,
            }],
            requirements: vec![
                LatticeNode {
                    id: "REQ-001".to_string(),
                    node_type: NodeType::Requirement,
                    title: "Test Requirement 1".to_string(),
                    body: "A test requirement".to_string(),
                    status: Status::Active,
                    version: "1.0.0".to_string(),
                    created_at: "2024-01-01".to_string(),
                    created_by: "test".to_string(),
                    requested_by: None,
                    priority: Some(Priority::P0),
                    category: None,
                    tags: None,
                    acceptance: None,
                    visibility: None,
                    resolution: Some(ResolutionInfo {
                        status: Resolution::Verified,
                        reason: None,
                        resolved_at: "2024-01-02".to_string(),
                        resolved_by: "test".to_string(),
                    }),
                    meta: None,
                    edges: None,
                },
                LatticeNode {
                    id: "REQ-002".to_string(),
                    node_type: NodeType::Requirement,
                    title: "Test Requirement 2".to_string(),
                    body: "Another test requirement".to_string(),
                    status: Status::Active,
                    version: "1.0.0".to_string(),
                    created_at: "2024-01-01".to_string(),
                    created_by: "test".to_string(),
                    requested_by: None,
                    priority: Some(Priority::P1),
                    category: None,
                    tags: None,
                    acceptance: None,
                    visibility: None,
                    resolution: None,
                    meta: None,
                    edges: None,
                },
            ],
            implementations: vec![],
        }
    }

    #[test]
    fn test_compute_statistics() {
        let data = create_test_data();
        let stats = compute_statistics(&data);

        assert_eq!(stats.sources, 1);
        assert_eq!(stats.theses, 1);
        assert_eq!(stats.requirements, 2);
        assert_eq!(stats.implementations, 0);
        assert_eq!(stats.verified, 1);
        assert_eq!(stats.unresolved, 1);
        assert_eq!(stats.p0, 1);
        assert_eq!(stats.p1, 1);
    }

    #[test]
    fn test_build_traceability_tree() {
        let data = create_test_data();
        let tree = build_traceability_tree(&data);

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].id, "THX-001");
    }
}
