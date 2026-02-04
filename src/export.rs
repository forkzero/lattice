//! Narrative export for Lattice.
//!
//! Linked requirements: REQ-CORE-009

use crate::types::{LatticeNode, Priority};
use std::collections::{HashMap, HashSet};

/// Target audience for narrative export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Investor,
    Contributor,
    Overview,
}

impl std::str::FromStr for Audience {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "investor" => Ok(Audience::Investor),
            "contributor" => Ok(Audience::Contributor),
            "overview" => Ok(Audience::Overview),
            _ => Err(format!("Invalid audience: {}", s)),
        }
    }
}

/// Options for narrative export.
pub struct ExportOptions {
    pub audience: Audience,
    pub title: String,
    pub include_internal: bool,
}

/// Lattice data for export.
pub struct LatticeData {
    pub sources: Vec<LatticeNode>,
    pub theses: Vec<LatticeNode>,
    pub requirements: Vec<LatticeNode>,
    pub implementations: Vec<LatticeNode>,
}

fn is_visible(node: &LatticeNode, include_internal: bool) -> bool {
    if include_internal {
        return true;
    }
    node.visibility.as_deref() != Some("internal")
}

fn get_sources_for_thesis<'a>(
    thesis: &LatticeNode,
    sources: &'a [LatticeNode],
) -> Vec<&'a LatticeNode> {
    let supported_by: HashSet<_> = thesis
        .edges
        .as_ref()
        .and_then(|e| e.supported_by.as_ref())
        .map(|refs| refs.iter().map(|r| r.target.as_str()).collect())
        .unwrap_or_default();

    sources
        .iter()
        .filter(|s| supported_by.contains(s.id.as_str()))
        .collect()
}

fn count_implemented(
    requirements: &[LatticeNode],
    implementations: &[LatticeNode],
) -> (usize, usize) {
    let mut implemented_ids: HashSet<String> = HashSet::new();

    for impl_node in implementations {
        if let Some(satisfies) = impl_node.edges.as_ref().and_then(|e| e.satisfies.as_ref()) {
            for edge in satisfies {
                implemented_ids.insert(edge.target.clone());
            }
        }
    }

    let implemented = requirements
        .iter()
        .filter(|r| implemented_ids.contains(&r.id))
        .count();

    (implemented, requirements.len())
}

fn group_by_priority(requirements: &[LatticeNode]) -> HashMap<Priority, Vec<&LatticeNode>> {
    let mut groups: HashMap<Priority, Vec<&LatticeNode>> = HashMap::new();
    groups.insert(Priority::P0, Vec::new());
    groups.insert(Priority::P1, Vec::new());
    groups.insert(Priority::P2, Vec::new());

    for req in requirements {
        let priority = req.priority.clone().unwrap_or(Priority::P2);
        groups.get_mut(&priority).unwrap().push(req);
    }

    groups
}

fn generate_investor_narrative(data: &LatticeData, options: &ExportOptions) -> String {
    let visible_theses: Vec<_> = data
        .theses
        .iter()
        .filter(|t| is_visible(t, options.include_internal))
        .collect();
    let visible_reqs: Vec<_> = data
        .requirements
        .iter()
        .filter(|r| is_visible(r, options.include_internal))
        .cloned()
        .collect();
    let (implemented, total) = count_implemented(&visible_reqs, &data.implementations);

    let mut lines = Vec::new();

    // Header
    lines.push(format!("# {}", options.title));
    lines.push(String::new());

    // Executive summary
    if let Some(main_thesis) = visible_theses.first() {
        let summary = main_thesis.body.lines().next().unwrap_or("").trim();
        lines.push(format!("> *{}*", summary));
        lines.push(String::new());
    }

    lines.push("---".to_string());
    lines.push(String::new());

    // Strategic Thesis section
    lines.push("## Strategic Thesis".to_string());
    lines.push(String::new());

    for thesis in &visible_theses {
        lines.push(format!("### {}", thesis.title));
        lines.push(String::new());
        lines.push(thesis.body.trim().to_string());
        lines.push(String::new());

        let supporting_sources = get_sources_for_thesis(thesis, &data.sources);
        if !supporting_sources.is_empty() {
            lines.push("**Research Support:**".to_string());
            for source in supporting_sources {
                let citation = source
                    .meta
                    .as_ref()
                    .and_then(|m| {
                        if let crate::types::NodeMeta::Source(sm) = m {
                            sm.citations
                                .as_ref()
                                .and_then(|c| c.first().cloned())
                                .or_else(|| sm.url.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "No citation".to_string());
                lines.push(format!("- {} ({})", source.title, citation));
            }
            lines.push(String::new());
        }

        lines.push("---".to_string());
        lines.push(String::new());
    }

    // What We're Building
    lines.push("## What We're Building".to_string());
    lines.push(String::new());

    let by_priority = group_by_priority(&visible_reqs);

    if !by_priority[&Priority::P0].is_empty() {
        lines.push("### Core Platform (P0 — MVP)".to_string());
        lines.push(String::new());
        lines.push("| Requirement | Description |".to_string());
        lines.push("|-------------|-------------|".to_string());
        for req in &by_priority[&Priority::P0] {
            let desc: String = req
                .body
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .chars()
                .take(80)
                .collect();
            lines.push(format!("| {} | {} |", req.title, desc));
        }
        lines.push(String::new());
    }

    if !by_priority[&Priority::P1].is_empty() {
        lines.push("### Extended Features (P1 — Beta)".to_string());
        lines.push(String::new());
        lines.push("| Requirement | Description |".to_string());
        lines.push("|-------------|-------------|".to_string());
        for req in &by_priority[&Priority::P1] {
            let desc: String = req
                .body
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .chars()
                .take(80)
                .collect();
            lines.push(format!("| {} | {} |", req.title, desc));
        }
        lines.push(String::new());
    }

    if !by_priority[&Priority::P2].is_empty() {
        lines.push("### Future Enhancements (P2)".to_string());
        lines.push(String::new());
        lines.push("| Requirement | Description |".to_string());
        lines.push("|-------------|-------------|".to_string());
        for req in &by_priority[&Priority::P2] {
            let desc: String = req
                .body
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .chars()
                .take(80)
                .collect();
            lines.push(format!("| {} | {} |", req.title, desc));
        }
        lines.push(String::new());
    }

    // Progress
    lines.push("## Progress".to_string());
    lines.push(String::new());
    let pct = if total > 0 {
        (implemented * 100) / total
    } else {
        0
    };
    lines.push(format!(
        "**{} of {} requirements implemented ({}%)**",
        implemented, total, pct
    ));
    lines.push(String::new());

    if !data.implementations.is_empty() {
        lines.push("### Implementations".to_string());
        lines.push(String::new());
        for impl_node in &data.implementations {
            let satisfies_count = impl_node
                .edges
                .as_ref()
                .and_then(|e| e.satisfies.as_ref())
                .map(|s| s.len())
                .unwrap_or(0);
            lines.push(format!(
                "- **{}** — satisfies {} requirement(s)",
                impl_node.title, satisfies_count
            ));
        }
        lines.push(String::new());
    }

    // Footer
    lines.push("---".to_string());
    lines.push(String::new());
    lines.push("*This document was auto-generated from the Lattice knowledge graph.*".to_string());

    lines.join("\n")
}

fn generate_contributor_narrative(data: &LatticeData, options: &ExportOptions) -> String {
    let visible_reqs: Vec<_> = data
        .requirements
        .iter()
        .filter(|r| is_visible(r, options.include_internal))
        .cloned()
        .collect();
    let (implemented, total) = count_implemented(&visible_reqs, &data.implementations);

    let mut implemented_ids: HashSet<String> = HashSet::new();
    for impl_node in &data.implementations {
        if let Some(satisfies) = impl_node.edges.as_ref().and_then(|e| e.satisfies.as_ref()) {
            for edge in satisfies {
                implemented_ids.insert(edge.target.clone());
            }
        }
    }

    let mut lines = Vec::new();

    lines.push(format!("# Contributing to {}", options.title));
    lines.push(String::new());
    lines.push(format!(
        "**{} requirements need implementation**",
        total - implemented
    ));
    lines.push(String::new());
    lines.push("---".to_string());
    lines.push(String::new());

    // Open requirements
    lines.push("## Open Requirements".to_string());
    lines.push(String::new());

    let by_priority = group_by_priority(&visible_reqs);

    for priority in [Priority::P0, Priority::P1, Priority::P2] {
        let reqs: Vec<_> = by_priority[&priority]
            .iter()
            .filter(|r| !implemented_ids.contains(&r.id))
            .collect();

        if !reqs.is_empty() {
            lines.push(format!("### {:?} Priority", priority));
            lines.push(String::new());

            for req in reqs {
                lines.push(format!("#### {}: {}", req.id, req.title));
                lines.push(String::new());
                lines.push(req.body.trim().to_string());
                lines.push(String::new());

                if req
                    .acceptance
                    .as_ref()
                    .map(|a| !a.is_empty())
                    .unwrap_or(false)
                {
                    let acceptance = req.acceptance.as_ref().unwrap();
                    lines.push("**Acceptance Criteria:**".to_string());
                    for test in acceptance {
                        lines.push(format!(
                            "- GIVEN {} WHEN {} THEN {}",
                            test.given, test.when, test.then
                        ));
                    }
                    lines.push(String::new());
                }
            }
        }
    }

    // Completed
    lines.push("## Completed".to_string());
    lines.push(String::new());
    for req in visible_reqs
        .iter()
        .filter(|r| implemented_ids.contains(&r.id))
    {
        lines.push(format!("- {} {}: {}", "\u{2705}", req.id, req.title));
    }
    lines.push(String::new());

    lines.push("---".to_string());
    lines.push(String::new());
    lines.push("*This document was auto-generated from the Lattice knowledge graph.*".to_string());

    lines.join("\n")
}

fn generate_overview_narrative(data: &LatticeData, options: &ExportOptions) -> String {
    let visible_theses: Vec<_> = data
        .theses
        .iter()
        .filter(|t| is_visible(t, options.include_internal))
        .collect();
    let visible_reqs: Vec<_> = data
        .requirements
        .iter()
        .filter(|r| is_visible(r, options.include_internal))
        .cloned()
        .collect();
    let (implemented, total) = count_implemented(&visible_reqs, &data.implementations);

    let mut lines = Vec::new();

    lines.push(format!("# {} Overview", options.title));
    lines.push(String::new());

    // Why
    if !visible_theses.is_empty() {
        lines.push("## Why".to_string());
        lines.push(String::new());
        for thesis in visible_theses.iter().take(3) {
            lines.push(format!("- **{}**", thesis.title));
        }
        lines.push(String::new());
    }

    // What
    lines.push("## What".to_string());
    lines.push(String::new());

    let categories: HashSet<_> = visible_reqs
        .iter()
        .filter_map(|r| r.category.as_ref())
        .collect();
    lines.push(format!(
        "{} requirements across {} categories.",
        visible_reqs.len(),
        categories.len()
    ));
    lines.push(String::new());

    let by_priority = group_by_priority(&visible_reqs);
    lines.push(format!(
        "- **P0 (MVP):** {} requirements",
        by_priority[&Priority::P0].len()
    ));
    lines.push(format!(
        "- **P1 (Beta):** {} requirements",
        by_priority[&Priority::P1].len()
    ));
    lines.push(format!(
        "- **P2 (Future):** {} requirements",
        by_priority[&Priority::P2].len()
    ));
    lines.push(String::new());

    // Progress
    lines.push("## Progress".to_string());
    lines.push(String::new());
    let pct = if total > 0 {
        (implemented * 100) / total
    } else {
        0
    };
    lines.push(format!("{}/{} implemented ({}%)", implemented, total, pct));
    lines.push(String::new());

    lines.push("---".to_string());
    lines.push(String::new());
    lines.push("*Auto-generated from the Lattice knowledge graph.*".to_string());

    lines.join("\n")
}

/// Export the lattice to a narrative document.
pub fn export_narrative(data: &LatticeData, options: &ExportOptions) -> String {
    match options.audience {
        Audience::Investor => generate_investor_narrative(data, options),
        Audience::Contributor => generate_contributor_narrative(data, options),
        Audience::Overview => generate_overview_narrative(data, options),
    }
}
