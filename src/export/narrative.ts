/**
 * Narrative export - generates human-readable documents from the lattice.
 *
 * Linked requirements: REQ-CORE-009
 */

import type {
  AnyLatticeNode,
  ThesisNode,
  SourceNode,
  RequirementNode,
  ImplementationNode,
} from '../core/types.js';

export type Audience = 'investor' | 'contributor' | 'overview';

interface ExportOptions {
  audience: Audience;
  includeInternal?: boolean;
  title?: string;
}

interface LatticeData {
  sources: SourceNode[];
  theses: ThesisNode[];
  requirements: RequirementNode[];
  implementations: ImplementationNode[];
}

/**
 * Check if a node should be included based on visibility.
 */
function isVisible(node: AnyLatticeNode, includeInternal: boolean): boolean {
  if (includeInternal) return true;
  const visibility = (node as any).visibility;
  return visibility !== 'internal';
}

/**
 * Get sources that support a thesis.
 */
function getSourcesForThesis(
  thesis: ThesisNode,
  sources: SourceNode[]
): SourceNode[] {
  const supportedBy = thesis.edges?.supported_by || [];
  const sourceIds = supportedBy.map((e) => e.target);
  return sources.filter((s) => sourceIds.includes(s.id));
}

/**
 * Count implemented requirements.
 */
function countImplemented(
  requirements: RequirementNode[],
  implementations: ImplementationNode[]
): { implemented: number; total: number } {
  const implementedIds = new Set<string>();

  for (const impl of implementations) {
    const satisfies = impl.edges?.satisfies || [];
    for (const edge of satisfies) {
      implementedIds.add(edge.target);
    }
  }

  const implemented = requirements.filter((r) =>
    implementedIds.has(r.id)
  ).length;

  return { implemented, total: requirements.length };
}

/**
 * Group requirements by priority.
 */
function groupByPriority(
  requirements: RequirementNode[]
): Record<string, RequirementNode[]> {
  const groups: Record<string, RequirementNode[]> = {
    P0: [],
    P1: [],
    P2: [],
  };

  for (const req of requirements) {
    const priority = req.priority || 'P2';
    if (!groups[priority]) groups[priority] = [];
    groups[priority].push(req);
  }

  return groups;
}

/**
 * Generate investor-focused narrative.
 */
function generateInvestorNarrative(
  data: LatticeData,
  options: ExportOptions
): string {
  const { sources, theses, requirements, implementations } = data;
  const includeInternal = options.includeInternal || false;

  const visibleTheses = theses.filter((t) => isVisible(t, includeInternal));
  const visibleReqs = requirements.filter((r) => isVisible(r, includeInternal));
  const progress = countImplemented(visibleReqs, implementations);

  const lines: string[] = [];

  // Header
  lines.push(`# ${options.title || 'Lattice'}`);
  lines.push('');

  // Executive summary from theses
  if (visibleTheses.length > 0) {
    const mainThesis = visibleTheses[0];
    lines.push(`> *${mainThesis.body.split('\n')[0].trim()}*`);
    lines.push('');
  }

  lines.push('---');
  lines.push('');

  // Strategic Thesis section
  lines.push('## Strategic Thesis');
  lines.push('');

  for (const thesis of visibleTheses) {
    lines.push(`### ${thesis.title}`);
    lines.push('');
    lines.push(thesis.body.trim());
    lines.push('');

    // Supporting research
    const supportingSources = getSourcesForThesis(thesis, sources);
    if (supportingSources.length > 0) {
      lines.push('**Research Support:**');
      for (const source of supportingSources) {
        const citation =
          source.meta?.citations?.[0] || source.meta?.url || 'No citation';
        lines.push(`- ${source.title} (${citation})`);
      }
      lines.push('');
    }

    lines.push('---');
    lines.push('');
  }

  // What We're Building section
  lines.push('## What We\'re Building');
  lines.push('');

  const byPriority = groupByPriority(visibleReqs);

  if (byPriority.P0.length > 0) {
    lines.push('### Core Platform (P0 — MVP)');
    lines.push('');
    lines.push('| Requirement | Description |');
    lines.push('|-------------|-------------|');
    for (const req of byPriority.P0) {
      const desc = req.body.split('\n')[0].trim().substring(0, 80);
      lines.push(`| ${req.title} | ${desc} |`);
    }
    lines.push('');
  }

  if (byPriority.P1.length > 0) {
    lines.push('### Extended Features (P1 — Beta)');
    lines.push('');
    lines.push('| Requirement | Description |');
    lines.push('|-------------|-------------|');
    for (const req of byPriority.P1) {
      const desc = req.body.split('\n')[0].trim().substring(0, 80);
      lines.push(`| ${req.title} | ${desc} |`);
    }
    lines.push('');
  }

  if (byPriority.P2.length > 0) {
    lines.push('### Future Enhancements (P2)');
    lines.push('');
    lines.push('| Requirement | Description |');
    lines.push('|-------------|-------------|');
    for (const req of byPriority.P2) {
      const desc = req.body.split('\n')[0].trim().substring(0, 80);
      lines.push(`| ${req.title} | ${desc} |`);
    }
    lines.push('');
  }

  // Progress section
  lines.push('## Progress');
  lines.push('');
  const pct = Math.round((progress.implemented / progress.total) * 100) || 0;
  lines.push(
    `**${progress.implemented} of ${progress.total} requirements implemented (${pct}%)**`
  );
  lines.push('');

  if (implementations.length > 0) {
    lines.push('### Implementations');
    lines.push('');
    for (const impl of implementations) {
      const satisfies = impl.edges?.satisfies || [];
      lines.push(`- **${impl.title}** — satisfies ${satisfies.length} requirement(s)`);
    }
    lines.push('');
  }

  // Footer
  lines.push('---');
  lines.push('');
  lines.push(
    '*This document was auto-generated from the Lattice knowledge graph.*'
  );

  return lines.join('\n');
}

/**
 * Generate contributor-focused narrative.
 */
function generateContributorNarrative(
  data: LatticeData,
  options: ExportOptions
): string {
  const { requirements, implementations } = data;
  const includeInternal = options.includeInternal || false;

  const visibleReqs = requirements.filter((r) => isVisible(r, includeInternal));
  const progress = countImplemented(visibleReqs, implementations);

  const implementedIds = new Set<string>();
  for (const impl of implementations) {
    for (const edge of impl.edges?.satisfies || []) {
      implementedIds.add(edge.target);
    }
  }

  const lines: string[] = [];

  lines.push(`# Contributing to ${options.title || 'Lattice'}`);
  lines.push('');
  lines.push(
    `**${progress.total - progress.implemented} requirements need implementation**`
  );
  lines.push('');
  lines.push('---');
  lines.push('');

  // Unimplemented requirements
  lines.push('## Open Requirements');
  lines.push('');

  const byPriority = groupByPriority(visibleReqs);

  for (const priority of ['P0', 'P1', 'P2']) {
    const reqs = byPriority[priority]?.filter((r) => !implementedIds.has(r.id));
    if (reqs && reqs.length > 0) {
      lines.push(`### ${priority} Priority`);
      lines.push('');
      for (const req of reqs) {
        lines.push(`#### ${req.id}: ${req.title}`);
        lines.push('');
        lines.push(req.body.trim());
        lines.push('');
        if (req.acceptance && req.acceptance.length > 0) {
          lines.push('**Acceptance Criteria:**');
          for (const test of req.acceptance) {
            lines.push(`- GIVEN ${test.given} WHEN ${test.when} THEN ${test.then}`);
          }
          lines.push('');
        }
      }
    }
  }

  // Implemented
  lines.push('## Completed');
  lines.push('');
  for (const req of visibleReqs.filter((r) => implementedIds.has(r.id))) {
    lines.push(`- ✅ ${req.id}: ${req.title}`);
  }
  lines.push('');

  lines.push('---');
  lines.push('');
  lines.push(
    '*This document was auto-generated from the Lattice knowledge graph.*'
  );

  return lines.join('\n');
}

/**
 * Generate overview narrative.
 */
function generateOverviewNarrative(
  data: LatticeData,
  options: ExportOptions
): string {
  const { theses, requirements, implementations } = data;
  const includeInternal = options.includeInternal || false;

  const visibleTheses = theses.filter((t) => isVisible(t, includeInternal));
  const visibleReqs = requirements.filter((r) => isVisible(r, includeInternal));
  const progress = countImplemented(visibleReqs, implementations);

  const lines: string[] = [];

  lines.push(`# ${options.title || 'Lattice'} Overview`);
  lines.push('');

  // Brief thesis summary
  if (visibleTheses.length > 0) {
    lines.push('## Why');
    lines.push('');
    for (const thesis of visibleTheses.slice(0, 3)) {
      lines.push(`- **${thesis.title}**`);
    }
    lines.push('');
  }

  // Requirements summary
  lines.push('## What');
  lines.push('');
  lines.push(`${visibleReqs.length} requirements across ${new Set(visibleReqs.map((r) => r.category)).size} categories.`);
  lines.push('');

  const byPriority = groupByPriority(visibleReqs);
  lines.push(`- **P0 (MVP):** ${byPriority.P0?.length || 0} requirements`);
  lines.push(`- **P1 (Beta):** ${byPriority.P1?.length || 0} requirements`);
  lines.push(`- **P2 (Future):** ${byPriority.P2?.length || 0} requirements`);
  lines.push('');

  // Progress
  lines.push('## Progress');
  lines.push('');
  const pct = Math.round((progress.implemented / progress.total) * 100) || 0;
  lines.push(`${progress.implemented}/${progress.total} implemented (${pct}%)`);
  lines.push('');

  lines.push('---');
  lines.push('');
  lines.push(
    '*Auto-generated from the Lattice knowledge graph.*'
  );

  return lines.join('\n');
}

/**
 * Export the lattice to a narrative document.
 */
export function exportNarrative(
  data: LatticeData,
  options: ExportOptions
): string {
  switch (options.audience) {
    case 'investor':
      return generateInvestorNarrative(data, options);
    case 'contributor':
      return generateContributorNarrative(data, options);
    case 'overview':
      return generateOverviewNarrative(data, options);
    default:
      return generateOverviewNarrative(data, options);
  }
}
