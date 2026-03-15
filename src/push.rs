//! Push lattice data to a remote API.
//!
//! Reads the local lattice graph, flattens embedded edges into a wire format,
//! and POSTs the result to a lattice-app server.

use serde::{Deserialize, Serialize};

use crate::diff::DiffResult;
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
    pub git_sha: String,
    pub nodes: Vec<PushNode>,
    pub edges: Vec<FlatEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<PushDiff>,
}

/// A single entry in the push diff payload.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PushDiffEntry {
    pub id: String,
    pub title: String,
    pub node_type: String,
    pub change_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,
}

/// Semantic diff included in the push payload.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PushDiff {
    pub base_ref: String,
    pub entries: Vec<PushDiffEntry>,
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
    #[serde(default)]
    pub last_push_sha: Option<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum PushError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Failed to serialize payload: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("Failed to compress payload: {0}")]
    Compress(#[from] std::io::Error),

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

/// Convert a `DiffResult` into a `PushDiff` for the wire format.
pub fn diff_result_to_push_diff(result: &DiffResult) -> PushDiff {
    let categories: &[(&[crate::diff::DiffEntry], &str)] = &[
        (&result.added, "added"),
        (&result.modified, "modified"),
        (&result.resolved, "resolved"),
        (&result.deleted, "deleted"),
    ];

    let entries = categories
        .iter()
        .flat_map(|(slice, change_type)| {
            slice.iter().map(move |e| PushDiffEntry {
                id: e.id.clone(),
                title: e.title.clone(),
                node_type: format!("{:?}", e.node_type).to_lowercase(),
                change_type: change_type.to_string(),
                fields: e.fields.clone(),
            })
        })
        .collect();

    PushDiff {
        base_ref: result.base_ref.clone(),
        entries,
    }
}

/// Fetch the last push SHA from the API. Returns None on any failure.
pub async fn fetch_last_push_sha(
    api_url: &str,
    api_key: &str,
    project_name: &str,
) -> Option<String> {
    let base = api_url.trim_end_matches('/');
    let url = reqwest::Url::parse_with_params(
        &format!("{}/api/lattice/last-push-sha", base),
        &[("projectName", project_name)],
    )
    .ok()?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    let resp = client
        .get(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ShaResponse {
        git_sha: Option<String>,
    }

    let body: ShaResponse = resp.json().await.ok()?;
    body.git_sha
}

/// Push lattice data to a remote API.
pub async fn push(
    api_url: &str,
    api_key: &str,
    project_name: &str,
    nodes: &[LatticeNode],
    git_sha: &str,
    diff: Option<PushDiff>,
) -> Result<PushResponse, PushError> {
    let push_nodes: Vec<PushNode> = nodes.iter().map(to_push_node).collect();
    let edges = flatten_edges(nodes);

    let payload = PushPayload {
        project_name: project_name.to_string(),
        git_sha: git_sha.to_string(),
        nodes: push_nodes,
        edges,
        diff,
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let url = format!("{}/api/lattice/push", api_url.trim_end_matches('/'));

    let json_bytes = serde_json::to_vec(&payload)?;

    let compressed = {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&json_bytes)?;
        encoder.finish()?
    };

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .header("Content-Encoding", "gzip")
        .body(compressed)
        .send()
        .await?;

    let status = resp.status().as_u16();
    if status != 200 {
        let body = resp.text().await.unwrap_or_default();
        return Err(PushError::Api { status, body });
    }

    Ok(resp.json().await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{ChangeType, DiffEntry, DiffResult};
    use crate::types::NodeType;

    #[test]
    fn test_push_payload_serialization_with_diff() {
        let payload = PushPayload {
            project_name: "test".to_string(),
            git_sha: "abc123".to_string(),
            nodes: vec![],
            edges: vec![],
            diff: Some(PushDiff {
                base_ref: "def456".to_string(),
                entries: vec![PushDiffEntry {
                    id: "REQ-001".to_string(),
                    title: "Test".to_string(),
                    node_type: "requirement".to_string(),
                    change_type: "added".to_string(),
                    fields: None,
                }],
            }),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["gitSha"], "abc123");
        assert!(json["diff"].is_object());
        assert_eq!(json["diff"]["baseRef"], "def456");
        assert_eq!(json["diff"]["entries"][0]["changeType"], "added");
    }

    #[test]
    fn test_push_payload_serialization_without_diff() {
        let payload = PushPayload {
            project_name: "test".to_string(),
            git_sha: "abc123".to_string(),
            nodes: vec![],
            edges: vec![],
            diff: None,
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["gitSha"], "abc123");
        assert!(json.get("diff").is_none());
    }

    #[test]
    fn test_diff_result_to_push_diff() {
        let result = DiffResult {
            base_ref: "abc123".to_string(),
            added: vec![DiffEntry {
                id: "REQ-001".to_string(),
                title: "New req".to_string(),
                node_type: NodeType::Requirement,
                priority: None,
                resolution: None,
                change_type: ChangeType::Added,
                fields: None,
            }],
            modified: vec![DiffEntry {
                id: "THX-001".to_string(),
                title: "Updated thesis".to_string(),
                node_type: NodeType::Thesis,
                priority: None,
                resolution: None,
                change_type: ChangeType::Modified,
                fields: Some(vec!["title".to_string()]),
            }],
            resolved: vec![DiffEntry {
                id: "REQ-002".to_string(),
                title: "Done req".to_string(),
                node_type: NodeType::Requirement,
                priority: None,
                resolution: Some("verified".to_string()),
                change_type: ChangeType::Modified,
                fields: None,
            }],
            deleted: vec![DiffEntry {
                id: "SRC-001".to_string(),
                title: "Old source".to_string(),
                node_type: NodeType::Source,
                priority: None,
                resolution: None,
                change_type: ChangeType::Deleted,
                fields: None,
            }],
        };

        let push_diff = diff_result_to_push_diff(&result);
        assert_eq!(push_diff.base_ref, "abc123");
        assert_eq!(push_diff.entries.len(), 4);
        assert_eq!(push_diff.entries[0].change_type, "added");
        assert_eq!(push_diff.entries[1].change_type, "modified");
        assert_eq!(push_diff.entries[1].fields, Some(vec!["title".to_string()]));
        assert_eq!(push_diff.entries[2].change_type, "resolved");
        assert_eq!(push_diff.entries[3].change_type, "deleted");
    }

    #[test]
    fn test_push_response_with_last_push_sha() {
        let json =
            r#"{"projectId": 1, "nodesUpserted": 5, "edgesReplaced": 3, "lastPushSha": "abc123"}"#;
        let resp: PushResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.last_push_sha, Some("abc123".to_string()));
    }

    #[test]
    fn test_push_response_without_last_push_sha() {
        let json = r#"{"projectId": 1, "nodesUpserted": 5, "edgesReplaced": 3}"#;
        let resp: PushResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.last_push_sha, None);
    }
}
