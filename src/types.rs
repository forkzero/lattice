//! Core types for Lattice nodes and edges.
//!
//! Linked requirements: REQ-CORE-001, REQ-CORE-002

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node type enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Source,
    Thesis,
    Requirement,
    Implementation,
}

/// Priority levels for requirements.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Priority {
    P0,
    P1,
    P2,
}

/// Node status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Draft,
    Active,
    Deprecated,
    Superseded,
}

/// Resolution status for requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resolution {
    /// Requirement is satisfied and working
    Verified,
    /// External constraint prevents implementation (implies revisit)
    Blocked,
    /// User chose to postpone (implies revisit)
    Deferred,
    /// Will not implement (no revisit)
    Wontfix,
}

/// Resolution information for a requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionInfo {
    pub status: Resolution,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub resolved_at: String,
    pub resolved_by: String,
}

/// Reliability level for sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reliability {
    PeerReviewed,
    Industry,
    Blog,
    Unverified,
}

/// Thesis category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThesisCategory {
    ValueProp,
    Market,
    Technical,
    Risk,
    Competitive,
}

/// A version-bound edge reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeReference {
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

impl EdgeReference {
    pub fn version_or_default(&self) -> &str {
        self.version.as_deref().unwrap_or("1.0.0")
    }
}

/// Edges container for different relationship types.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edges {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_by: Option<Vec<EdgeReference>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derives_from: Option<Vec<EdgeReference>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<EdgeReference>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub satisfies: Option<Vec<EdgeReference>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<Vec<EdgeReference>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reveals_gap_in: Option<Vec<EdgeReference>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub challenges: Option<Vec<EdgeReference>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validates: Option<Vec<EdgeReference>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflicts_with: Option<Vec<EdgeReference>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<Vec<EdgeReference>>,
}

/// Acceptance test for requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceTest {
    pub id: String,
    pub given: String,
    pub when: String,
    pub then: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<String>,
}

/// File reference for implementations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRef {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<String>>,
}

/// Source node metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reliability: Option<Reliability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieved_at: Option<String>,
}

/// Thesis node metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThesisMeta {
    pub category: ThesisCategory,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

// Custom serialization for f64 to handle equality
impl ThesisMeta {
    pub fn confidence_value(&self) -> f64 {
        self.confidence.unwrap_or(0.8)
    }
}

/// Implementation node metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_command: Option<String>,
}

/// A generic lattice node that can be any type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatticeNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    pub title: String,
    pub body: String,
    pub status: Status,
    pub version: String,
    pub created_at: String,
    pub created_by: String,

    // Optional fields depending on node type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance: Option<Vec<AcceptanceTest>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,

    // Resolution status (for requirements)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<ResolutionInfo>,

    // Type-specific metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<NodeMeta>,

    // Edges
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edges: Option<Edges>,
}

/// Union type for node-specific metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NodeMeta {
    Source(SourceMeta),
    Thesis(ThesisMeta),
    Implementation(ImplementationMeta),
}

impl LatticeNode {
    /// Get all edge references from this node.
    pub fn all_edges(&self) -> Vec<&EdgeReference> {
        let mut refs = Vec::new();
        if let Some(edges) = &self.edges {
            if let Some(e) = &edges.supported_by {
                refs.extend(e.iter());
            }
            if let Some(e) = &edges.derives_from {
                refs.extend(e.iter());
            }
            if let Some(e) = &edges.depends_on {
                refs.extend(e.iter());
            }
            if let Some(e) = &edges.satisfies {
                refs.extend(e.iter());
            }
            if let Some(e) = &edges.extends {
                refs.extend(e.iter());
            }
            if let Some(e) = &edges.reveals_gap_in {
                refs.extend(e.iter());
            }
            if let Some(e) = &edges.challenges {
                refs.extend(e.iter());
            }
            if let Some(e) = &edges.validates {
                refs.extend(e.iter());
            }
            if let Some(e) = &edges.conflicts_with {
                refs.extend(e.iter());
            }
            if let Some(e) = &edges.supersedes {
                refs.extend(e.iter());
            }
        }
        refs
    }
}

/// Node index type alias.
pub type NodeIndex = HashMap<String, LatticeNode>;
