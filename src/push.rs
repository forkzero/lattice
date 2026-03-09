//! Push lattice data to a remote API.
//!
//! Reads the local lattice graph, flattens embedded edges into a wire format,
//! and POSTs the result to a lattice-app server.

use serde::{Deserialize, Serialize};

use crate::types::LatticeNode;

/// A flat edge in the wire format expected by the lattice-app API.
#[derive(Debug, Serialize)]
pub struct FlatEdge {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
}

/// Push payload matching the server's `LatticePushPayload` interface.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushPayload {
    pub project_name: String,
    pub nodes: Vec<PushNode>,
    pub edges: Vec<FlatEdge>,
}

/// Simplified node for the push payload.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Response from the push endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResponse {
    pub project_id: i64,
    pub nodes_upserted: usize,
    pub edges_replaced: usize,
}

#[derive(thiserror::Error, Debug)]
pub enum PushError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API returned {status}: {body}")]
    Api { status: u16, body: String },

    #[error(
        "No API URL configured. Set api_url in .lattice/config.yaml, pass --api-url, or set LATTICE_API_URL"
    )]
    NoApiUrl,

    #[error("No API key configured. Pass --api-key or set LATTICE_API_KEY")]
    NoApiKey,
}

/// Convert a `LatticeNode` to a `PushNode`.
fn to_push_node(node: &LatticeNode) -> PushNode {
    // Extract URL from source metadata if present
    let url = node.meta.as_ref().and_then(|m| {
        if let crate::types::NodeMeta::Source(s) = m {
            s.url.clone()
        } else {
            None
        }
    });

    PushNode {
        id: node.id.clone(),
        node_type: serde_json::to_value(&node.node_type)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        title: node.title.clone(),
        body: if node.body.is_empty() {
            None
        } else {
            Some(node.body.clone())
        },
        url,
        status: Some(
            serde_json::to_value(&node.status)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "active".to_string()),
        ),
        created_at: node.created_at.clone(),
        updated_at: node.created_at.clone(),
    }
}

/// Extract embedded edges from nodes into a flat list.
///
/// Edge direction mapping:
/// - `supported_by` is reversed: if A has `supported_by: [B]`, the flat edge is
///   `{ source: B, target: A, type: "supports" }` (B supports A).
/// - All other edge types preserve direction: if A has `derives_from: [B]`, the
///   flat edge is `{ source: A, target: B, type: "derives-from" }`.
pub fn flatten_edges(nodes: &[LatticeNode]) -> Vec<FlatEdge> {
    let mut flat = Vec::new();

    for node in nodes {
        let Some(edges) = &node.edges else {
            continue;
        };

        // supported_by is reversed: B supports A
        if let Some(refs) = &edges.supported_by {
            for r in refs {
                flat.push(FlatEdge {
                    source: r.target.clone(),
                    target: node.id.clone(),
                    edge_type: "supports".to_string(),
                });
            }
        }

        // All others: A -> B with kebab-case type name
        let forward_edges: &[(&Option<Vec<crate::types::EdgeReference>>, &str)] = &[
            (&edges.derives_from, "derives-from"),
            (&edges.satisfies, "satisfies"),
            (&edges.depends_on, "depends-on"),
            (&edges.extends, "extends"),
            (&edges.reveals_gap_in, "reveals-gap-in"),
            (&edges.challenges, "challenges"),
            (&edges.validates, "validates"),
            (&edges.conflicts_with, "conflicts-with"),
            (&edges.supersedes, "supersedes"),
        ];

        for (edge_refs, type_name) in forward_edges {
            if let Some(refs) = edge_refs {
                for r in refs {
                    flat.push(FlatEdge {
                        source: node.id.clone(),
                        target: r.target.clone(),
                        edge_type: type_name.to_string(),
                    });
                }
            }
        }
    }

    flat
}

/// Push lattice data to a remote API.
pub async fn push(
    api_url: &str,
    api_key: &str,
    project_name: &str,
    nodes: &[LatticeNode],
) -> Result<PushResponse, PushError> {
    let push_nodes: Vec<PushNode> = nodes.iter().map(to_push_node).collect();
    let edges = flatten_edges(nodes);

    let payload = PushPayload {
        project_name: project_name.to_string(),
        nodes: push_nodes,
        edges,
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let url = format!("{}/api/lattice/push", api_url.trim_end_matches('/'));

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await?;

    let status = resp.status().as_u16();
    if status != 200 {
        let body = resp.text().await.unwrap_or_default();
        return Err(PushError::Api { status, body });
    }

    Ok(resp.json().await?)
}
