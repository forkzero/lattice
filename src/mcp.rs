//! MCP Server implementation for Lattice.
//!
//! Exposes lattice tools via the Model Context Protocol for LLM integration.
//! Linked requirement: REQ-API-004

use crate::graph::{build_node_index, find_drift};
use crate::storage::{
    AddImplementationOptions, AddRequirementOptions, ResolveOptions, add_implementation,
    add_requirement, find_lattice_root, load_nodes_by_type, resolve_node,
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
        assert_eq!(tools.len(), 7);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"lattice_summary"));
        assert!(names.contains(&"lattice_list"));
        assert!(names.contains(&"lattice_get"));
        assert!(names.contains(&"lattice_drift"));
        assert!(names.contains(&"lattice_resolve"));
        assert!(names.contains(&"lattice_add_requirement"));
        assert!(names.contains(&"lattice_add_implementation"));
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
}
