//! MCP Server implementation for Lattice.
//!
//! Exposes lattice tools via the Model Context Protocol for LLM integration.
//! Linked requirement: REQ-API-004

use crate::graph::{build_node_index, find_drift};
use crate::storage::{
    AddImplementationOptions, AddRequirementOptions, GapType, RefineOptions, ResolveOptions,
    add_implementation, add_requirement, find_lattice_root, load_nodes_by_type, refine_requirement,
    resolve_node,
};
use crate::types::{Priority, Resolution, Status};
use rmcp::ServiceExt;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, InitializeResult,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

/// MCP Server for Lattice
#[derive(Clone)]
pub struct LatticeServer {
    root: PathBuf,
}

impl LatticeServer {
    /// Create a new LatticeServer with the given lattice root directory
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Create a server using the current working directory to find the lattice root
    pub fn from_cwd() -> Result<Self, String> {
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        let root = find_lattice_root(&cwd).ok_or("Not in a lattice directory")?;
        Ok(Self::new(root))
    }

    /// Get the lattice summary
    fn summary(&self) -> Result<Value, String> {
        let sources = load_nodes_by_type(&self.root, "sources").unwrap_or_default();
        let theses = load_nodes_by_type(&self.root, "theses").unwrap_or_default();
        let requirements = load_nodes_by_type(&self.root, "requirements").unwrap_or_default();
        let implementations = load_nodes_by_type(&self.root, "implementations").unwrap_or_default();

        let mut unresolved = 0;
        let mut verified = 0;
        let mut blocked = 0;
        let mut deferred = 0;
        let mut wontfix = 0;
        let mut p0 = 0;
        let mut p1 = 0;
        let mut p2 = 0;
        let mut orphaned_reqs: Vec<String> = Vec::new();

        for req in &requirements {
            match req.resolution.as_ref().map(|r| &r.status) {
                Some(Resolution::Verified) => verified += 1,
                Some(Resolution::Blocked) => blocked += 1,
                Some(Resolution::Deferred) => deferred += 1,
                Some(Resolution::Wontfix) => wontfix += 1,
                None => unresolved += 1,
            }

            match req.priority {
                Some(Priority::P0) => p0 += 1,
                Some(Priority::P1) => p1 += 1,
                Some(Priority::P2) => p2 += 1,
                None => {}
            }

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

        let drift_reports = find_drift(&self.root).unwrap_or_default();
        let has_drift = !drift_reports.is_empty();

        let thesis_ids: HashSet<_> = theses.iter().map(|t| t.id.clone()).collect();
        let mut referenced_theses: HashSet<String> = HashSet::new();
        for req in &requirements {
            if let Some(edges) = &req.edges
                && let Some(derives_from) = &edges.derives_from
            {
                for edge in derives_from {
                    referenced_theses.insert(edge.target.clone());
                }
            }
        }
        let orphaned_theses: Vec<_> = thesis_ids.difference(&referenced_theses).cloned().collect();

        Ok(json!({
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
        }))
    }

    /// List nodes by type
    fn list(&self, node_type: &str, status_filter: Option<&str>) -> Result<Value, String> {
        let type_name = match node_type {
            "sources" => "sources",
            "theses" => "theses",
            "requirements" => "requirements",
            "implementations" => "implementations",
            _ => return Err(format!("Unknown type: {}", node_type)),
        };

        let nodes = load_nodes_by_type(&self.root, type_name).map_err(|e| e.to_string())?;

        let result: Vec<Value> = nodes
            .iter()
            .filter(|n| {
                if let Some(filter) = status_filter {
                    match filter.to_lowercase().as_str() {
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
                        "unresolved" => n.resolution.is_none(),
                        _ => true,
                    }
                } else {
                    true
                }
            })
            .map(|n| {
                json!({
                    "id": n.id,
                    "title": n.title,
                    "version": n.version,
                    "status": format!("{:?}", n.status).to_lowercase(),
                    "resolution": n.resolution.as_ref().map(|r| format!("{:?}", r.status).to_lowercase())
                })
            })
            .collect();

        Ok(json!(result))
    }

    /// Get a specific node by ID
    fn get(&self, id: &str) -> Result<Value, String> {
        let index = build_node_index(&self.root).map_err(|e| e.to_string())?;

        if let Some(node) = index.get(id) {
            Ok(serde_json::to_value(node).map_err(|e| e.to_string())?)
        } else {
            Err(format!("Node not found: {}", id))
        }
    }

    /// Check for drift
    fn drift(&self) -> Result<Value, String> {
        let reports = find_drift(&self.root).map_err(|e| e.to_string())?;

        let result: Vec<Value> = reports
            .iter()
            .map(|r| {
                json!({
                    "node_id": r.node_id,
                    "node_type": r.node_type,
                    "stale_edges": r.drift_items.iter().map(|i| {
                        json!({
                            "target_id": i.target_id,
                            "bound_version": i.bound_version,
                            "current_version": i.current_version,
                            "severity": format!("{:?}", i.severity).to_lowercase()
                        })
                    }).collect::<Vec<_>>()
                })
            })
            .collect();

        Ok(json!({
            "has_drift": !reports.is_empty(),
            "reports": result
        }))
    }

    /// Resolve a requirement
    fn resolve_req(&self, id: &str, status: &str, reason: Option<&str>) -> Result<Value, String> {
        let resolution = match status.to_lowercase().as_str() {
            "verified" => Resolution::Verified,
            "blocked" => Resolution::Blocked,
            "deferred" => Resolution::Deferred,
            "wontfix" => Resolution::Wontfix,
            _ => {
                return Err(format!(
                    "Invalid status: {}. Must be verified, blocked, deferred, or wontfix",
                    status
                ));
            }
        };

        let resolved_by = format!("agent:mcp-{}", chrono::Utc::now().format("%Y-%m-%d"));

        let options = ResolveOptions {
            node_id: id.to_string(),
            resolution: resolution.clone(),
            reason: reason.map(|s| s.to_string()),
            resolved_by,
        };

        let path = resolve_node(&self.root, options).map_err(|e| e.to_string())?;

        Ok(json!({
            "success": true,
            "id": id,
            "status": format!("{:?}", resolution).to_lowercase(),
            "file": path.display().to_string()
        }))
    }

    /// Add a requirement
    fn add_req(&self, params: AddRequirementParams) -> Result<Value, String> {
        let priority = match params.priority.to_uppercase().as_str() {
            "P0" => Priority::P0,
            "P1" => Priority::P1,
            "P2" => Priority::P2,
            _ => return Err(format!("Invalid priority: {}", params.priority)),
        };

        let created_by = format!("agent:mcp-{}", chrono::Utc::now().format("%Y-%m-%d"));

        let options = AddRequirementOptions {
            id: params.id.clone(),
            title: params.title,
            body: params.body,
            priority,
            category: params.category,
            tags: params.tags,
            derives_from: params.derives_from,
            depends_on: params.depends_on,
            status: Status::Active,
            created_by,
        };

        let path = add_requirement(&self.root, options).map_err(|e| e.to_string())?;

        Ok(json!({
            "success": true,
            "id": params.id,
            "file": path.display().to_string()
        }))
    }

    /// Refine a requirement by creating a sub-requirement from a gap
    fn refine(&self, params: RefineParams) -> Result<Value, String> {
        let gap_type: GapType = params.gap_type.parse().map_err(|e: String| e)?;
        let created_by = format!("agent:mcp-{}", chrono::Utc::now().format("%Y-%m-%d"));

        let options = RefineOptions {
            parent_id: params.parent.clone(),
            gap_type,
            title: params.title,
            description: params.description,
            proposed: params.proposed,
            implementation_id: params.implementation,
            created_by,
        };

        let result = refine_requirement(&self.root, options).map_err(|e| e.to_string())?;

        Ok(json!({
            "success": true,
            "sub_requirement_id": result.sub_requirement_id,
            "file": result.sub_requirement_path.display().to_string(),
            "parent_updated": result.parent_updated,
            "implementation_updated": result.implementation_updated
        }))
    }

    /// Search across nodes with multiple filter criteria
    fn search(&self, params: SearchParams) -> Result<Value, String> {
        let node_type = params.node_type.as_deref().unwrap_or("requirements");
        let type_name = match node_type {
            "sources" => "sources",
            "theses" => "theses",
            "requirements" => "requirements",
            "implementations" => "implementations",
            _ => return Err(format!("Unknown type: {}", node_type)),
        };

        let nodes = load_nodes_by_type(&self.root, type_name).map_err(|e| e.to_string())?;

        // For graph proximity, build a set of related node IDs
        let related_ids: Option<HashSet<String>> = if let Some(ref related_to) = params.related_to {
            let index = build_node_index(&self.root).map_err(|e| e.to_string())?;
            if let Some(source_node) = index.get(related_to) {
                let mut related = HashSet::new();

                // Collect edge targets from the source node
                let mut source_targets: HashSet<String> = HashSet::new();
                if let Some(edges) = &source_node.edges {
                    if let Some(derives_from) = &edges.derives_from {
                        for edge in derives_from {
                            source_targets.insert(edge.target.clone());
                        }
                    }
                    if let Some(depends_on) = &edges.depends_on {
                        for edge in depends_on {
                            source_targets.insert(edge.target.clone());
                        }
                    }
                    if let Some(satisfies) = &edges.satisfies {
                        for edge in satisfies {
                            source_targets.insert(edge.target.clone());
                        }
                    }
                    if let Some(supported_by) = &edges.supported_by {
                        for edge in supported_by {
                            source_targets.insert(edge.target.clone());
                        }
                    }
                }

                // Find nodes sharing any of these edge targets
                for node in index.values() {
                    if node.id == *related_to {
                        continue; // Skip the source node itself
                    }
                    if let Some(edges) = &node.edges {
                        let mut node_targets: HashSet<String> = HashSet::new();
                        if let Some(derives_from) = &edges.derives_from {
                            for edge in derives_from {
                                node_targets.insert(edge.target.clone());
                            }
                        }
                        if let Some(depends_on) = &edges.depends_on {
                            for edge in depends_on {
                                node_targets.insert(edge.target.clone());
                            }
                        }
                        if let Some(satisfies) = &edges.satisfies {
                            for edge in satisfies {
                                node_targets.insert(edge.target.clone());
                            }
                        }
                        if let Some(supported_by) = &edges.supported_by {
                            for edge in supported_by {
                                node_targets.insert(edge.target.clone());
                            }
                        }

                        // Check for intersection
                        if !source_targets.is_disjoint(&node_targets) {
                            related.insert(node.id.clone());
                        }
                    }
                }

                // Also include nodes that the source directly references or is referenced by
                for target in &source_targets {
                    related.insert(target.clone());
                }

                // Find nodes that reference the source node
                for node in index.values() {
                    if let Some(edges) = &node.edges {
                        let targets_source = edges
                            .derives_from
                            .as_ref()
                            .map(|v| v.iter().any(|e| e.target == *related_to))
                            .unwrap_or(false)
                            || edges
                                .depends_on
                                .as_ref()
                                .map(|v| v.iter().any(|e| e.target == *related_to))
                                .unwrap_or(false)
                            || edges
                                .satisfies
                                .as_ref()
                                .map(|v| v.iter().any(|e| e.target == *related_to))
                                .unwrap_or(false)
                            || edges
                                .supported_by
                                .as_ref()
                                .map(|v: &Vec<_>| v.iter().any(|e| e.target == *related_to))
                                .unwrap_or(false);
                        if targets_source {
                            related.insert(node.id.clone());
                        }
                    }
                }

                Some(related)
            } else {
                return Err(format!("Node not found: {}", related_to));
            }
        } else {
            None
        };

        let results: Vec<Value> = nodes
            .iter()
            .filter(|n| {
                // ID prefix filter
                if let Some(ref prefix) = params.id_prefix
                    && !n.id.to_uppercase().starts_with(&prefix.to_uppercase())
                {
                    return false;
                }

                // Graph proximity filter
                if let Some(ref related) = related_ids
                    && !related.contains(&n.id)
                {
                    return false;
                }

                // Text search in title and body
                if let Some(ref query) = params.query {
                    let query_lower = query.to_lowercase();
                    let title_match = n.title.to_lowercase().contains(&query_lower);
                    let body_match = n.body.to_lowercase().contains(&query_lower);
                    if !title_match && !body_match {
                        return false;
                    }
                }

                // Priority filter
                if let Some(ref priority) = params.priority {
                    let node_priority = n.priority.as_ref().map(|p| format!("{:?}", p));
                    if node_priority.as_deref() != Some(priority.to_uppercase().as_str()) {
                        return false;
                    }
                }

                // Resolution status filter
                if let Some(ref resolution) = params.resolution {
                    let res_lower = resolution.to_lowercase();
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

                // Single tag filter (backwards compatible)
                if let Some(ref tag) = params.tag {
                    let tag_lower = tag.to_lowercase();
                    let has_tag = n
                        .tags
                        .as_ref()
                        .map(|tags| tags.iter().any(|t| t.to_lowercase() == tag_lower))
                        .unwrap_or(false);
                    if !has_tag {
                        return false;
                    }
                }

                // Tags intersection filter (all specified tags must be present)
                if let Some(ref search_tags) = params.tags {
                    let node_tags: HashSet<String> = n
                        .tags
                        .as_ref()
                        .map(|tags| tags.iter().map(|t| t.to_lowercase()).collect())
                        .unwrap_or_default();
                    for search_tag in search_tags {
                        if !node_tags.contains(&search_tag.to_lowercase()) {
                            return false;
                        }
                    }
                }

                // Category filter
                if let Some(ref category) = params.category {
                    let cat_lower = category.to_lowercase();
                    let matches_cat = n
                        .category
                        .as_ref()
                        .map(|c| c.to_lowercase() == cat_lower)
                        .unwrap_or(false);
                    if !matches_cat {
                        return false;
                    }
                }

                true
            })
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

        Ok(json!({
            "count": results.len(),
            "results": results
        }))
    }

    /// Add an implementation
    fn add_impl(&self, params: AddImplementationParams) -> Result<Value, String> {
        let created_by = format!("agent:mcp-{}", chrono::Utc::now().format("%Y-%m-%d"));

        let options = AddImplementationOptions {
            id: params.id.clone(),
            title: params.title,
            body: params.body,
            language: params.language,
            files: params.files,
            test_command: params.test_command,
            satisfies: params.satisfies,
            status: Status::Active,
            created_by,
        };

        let path = add_implementation(&self.root, options).map_err(|e| e.to_string())?;

        Ok(json!({
            "success": true,
            "id": params.id,
            "file": path.display().to_string()
        }))
    }
}

#[derive(Debug, Deserialize)]
struct AddRequirementParams {
    id: String,
    title: String,
    body: String,
    priority: String,
    category: String,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    derives_from: Option<Vec<String>>,
    #[serde(default)]
    depends_on: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct AddImplementationParams {
    id: String,
    title: String,
    body: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    files: Option<Vec<String>>,
    #[serde(default)]
    test_command: Option<String>,
    #[serde(default)]
    satisfies: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RefineParams {
    parent: String,
    gap_type: String,
    title: String,
    description: String,
    #[serde(default)]
    proposed: Option<String>,
    #[serde(default)]
    implementation: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchParams {
    #[serde(default)]
    node_type: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    id_prefix: Option<String>,
    #[serde(default)]
    related_to: Option<String>,
}

fn make_schema(properties: Value, required: Vec<&str>) -> Arc<Map<String, Value>> {
    let mut schema = Map::new();
    schema.insert("type".to_string(), json!("object"));
    schema.insert("properties".to_string(), properties);
    if !required.is_empty() {
        schema.insert(
            "required".to_string(),
            json!(required.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
        );
    }
    Arc::new(schema)
}

/// Tool definitions for MCP
fn get_tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "lattice_summary",
            "Get a compact overview of the knowledge graph: node counts by type, \
             requirement resolution status distribution, priority breakdown, drift status, \
             and orphaned nodes. Use this first to understand the current state.",
            make_schema(json!({}), vec![]),
        ),
        Tool::new(
            "lattice_list",
            "List nodes by type (sources, theses, requirements, implementations). \
             Returns ID, title, version, and status for each.",
            make_schema(
                json!({
                    "type": {
                        "type": "string",
                        "description": "Node type: sources, theses, requirements, or implementations",
                        "enum": ["sources", "theses", "requirements", "implementations"]
                    },
                    "status": {
                        "type": "string",
                        "description": "Optional filter: verified, blocked, deferred, wontfix, unresolved",
                        "enum": ["verified", "blocked", "deferred", "wontfix", "unresolved"]
                    }
                }),
                vec!["type"],
            ),
        ),
        Tool::new(
            "lattice_get",
            "Retrieve full details of a specific node by ID. Returns title, body, \
             version, edges, resolution status, and all metadata.",
            make_schema(
                json!({
                    "id": {
                        "type": "string",
                        "description": "Node ID (e.g., REQ-API-004, THX-AGENT-NATIVE-TOOLS)"
                    }
                }),
                vec!["id"],
            ),
        ),
        Tool::new(
            "lattice_drift",
            "Check for version drift in the knowledge graph. Identifies edges where \
             the target node version has changed since the edge was created.",
            make_schema(json!({}), vec![]),
        ),
        Tool::new(
            "lattice_resolve",
            "Update the resolution status of a requirement. Statuses: verified \
             (implemented and tested), blocked (waiting on dependency), deferred \
             (future work), wontfix (rejected).",
            make_schema(
                json!({
                    "id": {
                        "type": "string",
                        "description": "Requirement ID (e.g., REQ-API-004)"
                    },
                    "status": {
                        "type": "string",
                        "description": "Resolution status",
                        "enum": ["verified", "blocked", "deferred", "wontfix"]
                    },
                    "reason": {
                        "type": "string",
                        "description": "Optional reason for the resolution"
                    }
                }),
                vec!["id", "status"],
            ),
        ),
        Tool::new(
            "lattice_add_requirement",
            "Create a new requirement node. Links to thesis via derives_from edge.",
            make_schema(
                json!({
                    "id": {
                        "type": "string",
                        "description": "Requirement ID (e.g., REQ-API-005)"
                    },
                    "title": {
                        "type": "string",
                        "description": "Short descriptive title"
                    },
                    "body": {
                        "type": "string",
                        "description": "Detailed description of the requirement"
                    },
                    "priority": {
                        "type": "string",
                        "description": "Priority level",
                        "enum": ["P0", "P1", "P2"]
                    },
                    "category": {
                        "type": "string",
                        "description": "Category (e.g., API, CLI, CORE)"
                    },
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Optional tags"
                    },
                    "derives_from": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Thesis IDs this requirement derives from"
                    },
                    "depends_on": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Other requirement IDs this depends on"
                    }
                }),
                vec!["id", "title", "body", "priority", "category"],
            ),
        ),
        Tool::new(
            "lattice_add_implementation",
            "Create an implementation node tracking code that satisfies requirements.",
            make_schema(
                json!({
                    "id": {
                        "type": "string",
                        "description": "Implementation ID (e.g., IMP-MCP-001)"
                    },
                    "title": {
                        "type": "string",
                        "description": "Short descriptive title"
                    },
                    "body": {
                        "type": "string",
                        "description": "Description of what this implementation does"
                    },
                    "language": {
                        "type": "string",
                        "description": "Programming language (e.g., rust, python)"
                    },
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "File paths included in this implementation"
                    },
                    "test_command": {
                        "type": "string",
                        "description": "Command to run tests (e.g., cargo test)"
                    },
                    "satisfies": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Requirement IDs this implementation satisfies"
                    }
                }),
                vec!["id", "title", "body"],
            ),
        ),
        Tool::new(
            "lattice_refine",
            "Create a sub-requirement when you discover an ambiguity, gap, or contradiction \
             during implementation. This captures implicit decisions as explicit, reviewable \
             requirements. Gap types: clarification (low-stakes ambiguity, auto-resolves), \
             design_decision (multiple valid approaches, flagged for review), \
             missing_requirement (new capability needed), contradiction (requirements conflict, \
             escalates to human). Auto-generates sub-requirement ID ({PARENT}-A, -B, etc.), \
             wires depends_on from parent, and optionally adds reveals_gap_in from implementation.",
            make_schema(
                json!({
                    "parent": {
                        "type": "string",
                        "description": "Parent requirement ID (e.g., REQ-CORE-005)"
                    },
                    "gap_type": {
                        "type": "string",
                        "description": "Type of gap discovered",
                        "enum": ["clarification", "design_decision", "missing_requirement", "contradiction"]
                    },
                    "title": {
                        "type": "string",
                        "description": "Brief title for the sub-requirement"
                    },
                    "description": {
                        "type": "string",
                        "description": "What is underspecified and why it matters for implementation"
                    },
                    "proposed": {
                        "type": "string",
                        "description": "Your proposed resolution (always provide one, even for design decisions)"
                    },
                    "implementation": {
                        "type": "string",
                        "description": "Implementation ID that discovered this gap (adds reveals_gap_in edge)"
                    }
                }),
                vec!["parent", "gap_type", "title", "description"],
            ),
        ),
        Tool::new(
            "lattice_search",
            "Search for nodes with flexible filtering. Supports text search, priority/status filtering, \
             tag intersection, ID prefix matching, and graph proximity (find related nodes). \
             All filters are optional and combined with AND.",
            make_schema(
                json!({
                    "node_type": {
                        "type": "string",
                        "description": "Node type to search (default: requirements)",
                        "enum": ["sources", "theses", "requirements", "implementations"]
                    },
                    "query": {
                        "type": "string",
                        "description": "Text to search for in title and body (case-insensitive)"
                    },
                    "priority": {
                        "type": "string",
                        "description": "Filter by priority level",
                        "enum": ["P0", "P1", "P2"]
                    },
                    "resolution": {
                        "type": "string",
                        "description": "Filter by resolution status",
                        "enum": ["verified", "blocked", "deferred", "wontfix", "unresolved", "open"]
                    },
                    "tag": {
                        "type": "string",
                        "description": "Filter by single tag (exact match, case-insensitive)"
                    },
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Filter by multiple tags (intersection - all must match)"
                    },
                    "category": {
                        "type": "string",
                        "description": "Filter by category (e.g., API, CLI, CORE)"
                    },
                    "id_prefix": {
                        "type": "string",
                        "description": "Filter by ID prefix (e.g., 'REQ-API' matches REQ-API-001, REQ-API-002)"
                    },
                    "related_to": {
                        "type": "string",
                        "description": "Find nodes related to this ID via graph proximity (shared edges, dependencies)"
                    }
                }),
                vec![],
            ),
        ),
    ]
}

#[allow(clippy::manual_async_fn)]
impl ServerHandler for LatticeServer {
    fn get_info(&self) -> InitializeResult {
        let capabilities = ServerCapabilities::builder().enable_tools().build();

        InitializeResult {
            protocol_version: rmcp::model::ProtocolVersion::LATEST,
            capabilities,
            server_info: Implementation {
                name: "lattice".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Lattice Knowledge Graph".to_string()),
                website_url: Some("https://github.com/forkzero/lattice".to_string()),
                icons: None,
            },
            instructions: Some(
                "Lattice is a knowledge coordination protocol. Use lattice_summary first \
                 to understand the current state, then lattice_list and lattice_get for \
                 details. Use lattice_drift to check for version inconsistencies."
                    .to_string(),
            ),
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::model::ErrorData>> + Send + '_
    {
        async move {
            Ok(ListToolsResult {
                tools: get_tools(),
                next_cursor: None,
                meta: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::model::ErrorData>> + Send + '_
    {
        async move {
            let name = request.name.as_ref();
            let arguments = request.arguments.unwrap_or_default();

            let result = match name {
                "lattice_summary" => self.summary(),
                "lattice_list" => {
                    let node_type =
                        arguments
                            .get("type")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                rmcp::model::ErrorData::invalid_params(
                                    "Missing required parameter: type",
                                    None,
                                )
                            })?;
                    let status = arguments.get("status").and_then(|v| v.as_str());
                    self.list(node_type, status)
                }
                "lattice_get" => {
                    let id = arguments
                        .get("id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            rmcp::model::ErrorData::invalid_params(
                                "Missing required parameter: id",
                                None,
                            )
                        })?;
                    self.get(id)
                }
                "lattice_drift" => self.drift(),
                "lattice_resolve" => {
                    let id = arguments
                        .get("id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            rmcp::model::ErrorData::invalid_params(
                                "Missing required parameter: id",
                                None,
                            )
                        })?;
                    let status = arguments
                        .get("status")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            rmcp::model::ErrorData::invalid_params(
                                "Missing required parameter: status",
                                None,
                            )
                        })?;
                    let reason = arguments.get("reason").and_then(|v| v.as_str());
                    self.resolve_req(id, status, reason)
                }
                "lattice_add_requirement" => {
                    let params: AddRequirementParams = serde_json::from_value(
                        serde_json::to_value(&arguments).unwrap_or_default(),
                    )
                    .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
                    self.add_req(params)
                }
                "lattice_add_implementation" => {
                    let params: AddImplementationParams = serde_json::from_value(
                        serde_json::to_value(&arguments).unwrap_or_default(),
                    )
                    .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
                    self.add_impl(params)
                }
                "lattice_refine" => {
                    let params: RefineParams = serde_json::from_value(
                        serde_json::to_value(&arguments).unwrap_or_default(),
                    )
                    .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
                    self.refine(params)
                }
                "lattice_search" => {
                    let params: SearchParams = serde_json::from_value(
                        serde_json::to_value(&arguments).unwrap_or_default(),
                    )
                    .map_err(|e| rmcp::model::ErrorData::invalid_params(e.to_string(), None))?;
                    self.search(params)
                }
                _ => {
                    return Err(rmcp::model::ErrorData::invalid_params(
                        format!("Unknown tool: {}", name),
                        None,
                    ));
                }
            };

            match result {
                Ok(value) => Ok(CallToolResult {
                    content: vec![Content::text(
                        serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
                    )],
                    is_error: None,
                    meta: None,
                    structured_content: None,
                }),
                Err(e) => Ok(CallToolResult {
                    content: vec![Content::text(e)],
                    is_error: Some(true),
                    meta: None,
                    structured_content: None,
                }),
            }
        }
    }
}

/// Run the MCP server over stdio
pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    let server = LatticeServer::from_cwd()?;

    server
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init_lattice;
    use tempfile::TempDir;

    fn setup_test_lattice() -> (TempDir, LatticeServer) {
        let temp_dir = TempDir::new().unwrap();
        init_lattice(temp_dir.path(), false).unwrap();
        let server = LatticeServer::new(temp_dir.path().to_path_buf());
        (temp_dir, server)
    }

    #[test]
    fn test_get_tools_returns_all_tools() {
        let tools = get_tools();
        assert_eq!(tools.len(), 9);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"lattice_summary"));
        assert!(names.contains(&"lattice_list"));
        assert!(names.contains(&"lattice_get"));
        assert!(names.contains(&"lattice_drift"));
        assert!(names.contains(&"lattice_resolve"));
        assert!(names.contains(&"lattice_add_requirement"));
        assert!(names.contains(&"lattice_add_implementation"));
        assert!(names.contains(&"lattice_refine"));
        assert!(names.contains(&"lattice_search"));
    }

    #[test]
    fn test_summary_returns_valid_json() {
        let (_temp_dir, server) = setup_test_lattice();
        let result = server.summary();
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("nodes").is_some());
        assert!(value.get("requirements").is_some());
        assert!(value.get("drift").is_some());
    }

    #[test]
    fn test_list_requirements_empty_lattice() {
        let (_temp_dir, server) = setup_test_lattice();
        let result = server.list("requirements", None);
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.is_array());
        assert_eq!(value.as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_list_invalid_type() {
        let (_temp_dir, server) = setup_test_lattice();
        let result = server.list("invalid_type", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown type"));
    }

    #[test]
    fn test_get_nonexistent_node() {
        let (_temp_dir, server) = setup_test_lattice();
        let result = server.get("REQ-DOES-NOT-EXIST");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Node not found"));
    }

    #[test]
    fn test_drift_empty_lattice() {
        let (_temp_dir, server) = setup_test_lattice();
        let result = server.drift();
        assert!(result.is_ok());

        let value = result.unwrap();
        assert_eq!(value.get("has_drift").unwrap(), false);
        assert_eq!(value.get("reports").unwrap().as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_resolve_invalid_status() {
        let (_temp_dir, server) = setup_test_lattice();
        let result = server.resolve_req("REQ-TEST", "invalid_status", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid status"));
    }

    #[test]
    fn test_add_req_invalid_priority() {
        let (_temp_dir, server) = setup_test_lattice();
        let params = AddRequirementParams {
            id: "REQ-TEST-001".to_string(),
            title: "Test Requirement".to_string(),
            body: "Test body".to_string(),
            priority: "P5".to_string(), // Invalid
            category: "TEST".to_string(),
            tags: None,
            derives_from: None,
            depends_on: None,
        };
        let result = server.add_req(params);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid priority"));
    }

    #[test]
    fn test_add_req_success() {
        let (_temp_dir, server) = setup_test_lattice();
        let params = AddRequirementParams {
            id: "REQ-TEST-001".to_string(),
            title: "Test Requirement".to_string(),
            body: "Test body".to_string(),
            priority: "P1".to_string(),
            category: "TEST".to_string(),
            tags: None,
            derives_from: None,
            depends_on: None,
        };
        let result = server.add_req(params);
        assert!(result.is_ok());

        let value = result.unwrap();
        assert_eq!(value.get("success").unwrap(), true);
        assert_eq!(value.get("id").unwrap(), "REQ-TEST-001");
    }

    #[test]
    fn test_add_impl_success() {
        let (_temp_dir, server) = setup_test_lattice();
        let params = AddImplementationParams {
            id: "IMP-TEST-001".to_string(),
            title: "Test Implementation".to_string(),
            body: "Test body".to_string(),
            language: Some("rust".to_string()),
            files: Some(vec!["src/test.rs".to_string()]),
            test_command: Some("cargo test".to_string()),
            satisfies: None,
        };
        let result = server.add_impl(params);
        assert!(result.is_ok());

        let value = result.unwrap();
        assert_eq!(value.get("success").unwrap(), true);
        assert_eq!(value.get("id").unwrap(), "IMP-TEST-001");
    }

    #[test]
    fn test_search_empty_lattice() {
        let (_temp_dir, server) = setup_test_lattice();
        let params = SearchParams {
            node_type: Some("requirements".to_string()),
            query: None,
            priority: None,
            resolution: Some("unresolved".to_string()),
            tag: None,
            tags: None,
            category: None,
            id_prefix: None,
            related_to: None,
        };
        let result = server.search(params);
        assert!(result.is_ok());

        let value = result.unwrap();
        assert_eq!(value.get("count").unwrap(), 0);
    }

    #[test]
    fn test_search_with_results() {
        let (_temp_dir, server) = setup_test_lattice();

        // Add a requirement first
        let add_params = AddRequirementParams {
            id: "REQ-SEARCH-001".to_string(),
            title: "Searchable Requirement".to_string(),
            body: "This requirement is for testing search".to_string(),
            priority: "P0".to_string(),
            category: "TEST".to_string(),
            tags: Some(vec!["mvp".to_string(), "core".to_string()]),
            derives_from: None,
            depends_on: None,
        };
        server.add_req(add_params).unwrap();

        // Search by text
        let search_params = SearchParams {
            node_type: Some("requirements".to_string()),
            query: Some("searchable".to_string()),
            priority: None,
            resolution: None,
            tag: None,
            tags: None,
            category: None,
            id_prefix: None,
            related_to: None,
        };
        let result = server.search(search_params).unwrap();
        assert_eq!(result.get("count").unwrap(), 1);

        // Search by priority
        let search_params = SearchParams {
            node_type: Some("requirements".to_string()),
            query: None,
            priority: Some("P0".to_string()),
            resolution: None,
            tag: None,
            tags: None,
            category: None,
            id_prefix: None,
            related_to: None,
        };
        let result = server.search(search_params).unwrap();
        assert_eq!(result.get("count").unwrap(), 1);

        // Search by tag
        let search_params = SearchParams {
            node_type: Some("requirements".to_string()),
            query: None,
            priority: None,
            resolution: None,
            tag: Some("mvp".to_string()),
            tags: None,
            category: None,
            id_prefix: None,
            related_to: None,
        };
        let result = server.search(search_params).unwrap();
        assert_eq!(result.get("count").unwrap(), 1);

        // Search with no matches
        let search_params = SearchParams {
            node_type: Some("requirements".to_string()),
            query: Some("nonexistent".to_string()),
            priority: None,
            resolution: None,
            tag: None,
            tags: None,
            category: None,
            id_prefix: None,
            related_to: None,
        };
        let result = server.search(search_params).unwrap();
        assert_eq!(result.get("count").unwrap(), 0);
    }

    #[test]
    fn test_search_by_id_prefix() {
        let (_temp_dir, server) = setup_test_lattice();

        // Add requirements with different prefixes
        server
            .add_req(AddRequirementParams {
                id: "REQ-API-001".to_string(),
                title: "API Requirement".to_string(),
                body: "API body".to_string(),
                priority: "P1".to_string(),
                category: "API".to_string(),
                tags: None,
                derives_from: None,
                depends_on: None,
            })
            .unwrap();
        server
            .add_req(AddRequirementParams {
                id: "REQ-CLI-001".to_string(),
                title: "CLI Requirement".to_string(),
                body: "CLI body".to_string(),
                priority: "P1".to_string(),
                category: "CLI".to_string(),
                tags: None,
                derives_from: None,
                depends_on: None,
            })
            .unwrap();

        // Search by ID prefix
        let search_params = SearchParams {
            node_type: Some("requirements".to_string()),
            query: None,
            priority: None,
            resolution: None,
            tag: None,
            tags: None,
            category: None,
            id_prefix: Some("REQ-API".to_string()),
            related_to: None,
        };
        let result = server.search(search_params).unwrap();
        assert_eq!(result.get("count").unwrap(), 1);

        // Search with broader prefix
        let search_params = SearchParams {
            node_type: Some("requirements".to_string()),
            query: None,
            priority: None,
            resolution: None,
            tag: None,
            tags: None,
            category: None,
            id_prefix: Some("REQ".to_string()),
            related_to: None,
        };
        let result = server.search(search_params).unwrap();
        assert_eq!(result.get("count").unwrap(), 2);
    }

    #[test]
    fn test_search_by_tags_intersection() {
        let (_temp_dir, server) = setup_test_lattice();

        // Add requirements with different tags
        server
            .add_req(AddRequirementParams {
                id: "REQ-TAGS-001".to_string(),
                title: "Has both tags".to_string(),
                body: "Body".to_string(),
                priority: "P1".to_string(),
                category: "TEST".to_string(),
                tags: Some(vec!["mvp".to_string(), "core".to_string()]),
                derives_from: None,
                depends_on: None,
            })
            .unwrap();
        server
            .add_req(AddRequirementParams {
                id: "REQ-TAGS-002".to_string(),
                title: "Has only mvp".to_string(),
                body: "Body".to_string(),
                priority: "P1".to_string(),
                category: "TEST".to_string(),
                tags: Some(vec!["mvp".to_string()]),
                derives_from: None,
                depends_on: None,
            })
            .unwrap();

        // Search requiring both tags (intersection)
        let search_params = SearchParams {
            node_type: Some("requirements".to_string()),
            query: None,
            priority: None,
            resolution: None,
            tag: None,
            tags: Some(vec!["mvp".to_string(), "core".to_string()]),
            category: None,
            id_prefix: None,
            related_to: None,
        };
        let result = server.search(search_params).unwrap();
        assert_eq!(result.get("count").unwrap(), 1);

        // Search with single tag in array
        let search_params = SearchParams {
            node_type: Some("requirements".to_string()),
            query: None,
            priority: None,
            resolution: None,
            tag: None,
            tags: Some(vec!["mvp".to_string()]),
            category: None,
            id_prefix: None,
            related_to: None,
        };
        let result = server.search(search_params).unwrap();
        assert_eq!(result.get("count").unwrap(), 2);
    }

    #[test]
    fn test_refine_creates_sub_requirement() {
        let (_temp_dir, server) = setup_test_lattice();

        // Create parent requirement
        server
            .add_req(AddRequirementParams {
                id: "REQ-REF-001".to_string(),
                title: "Refinable Requirement".to_string(),
                body: "A requirement that needs refinement".to_string(),
                priority: "P0".to_string(),
                category: "TEST".to_string(),
                tags: None,
                derives_from: None,
                depends_on: None,
            })
            .unwrap();

        // Refine it via MCP
        let result = server
            .refine(RefineParams {
                parent: "REQ-REF-001".to_string(),
                gap_type: "design_decision".to_string(),
                title: "Error format".to_string(),
                description: "Need to decide error format".to_string(),
                proposed: Some("Use JSON".to_string()),
                implementation: None,
            })
            .unwrap();

        assert_eq!(result.get("success").unwrap(), true);
        assert_eq!(result.get("sub_requirement_id").unwrap(), "REQ-REF-001-A");
        assert_eq!(result.get("parent_updated").unwrap(), true);
    }

    #[test]
    fn test_refine_invalid_gap_type() {
        let (_temp_dir, server) = setup_test_lattice();

        server
            .add_req(AddRequirementParams {
                id: "REQ-BAD-001".to_string(),
                title: "Bad Gap Type".to_string(),
                body: "Body".to_string(),
                priority: "P0".to_string(),
                category: "TEST".to_string(),
                tags: None,
                derives_from: None,
                depends_on: None,
            })
            .unwrap();

        let result = server.refine(RefineParams {
            parent: "REQ-BAD-001".to_string(),
            gap_type: "invalid_type".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            proposed: None,
            implementation: None,
        });

        assert!(result.is_err());
    }
}
