/**
 * Graph traversal and drift detection.
 *
 * Linked requirements: REQ-CORE-003, REQ-CORE-005
 */

import type { AnyLatticeNode, EdgeReference } from '../core/types.js';
import { loadNodesByType } from '../storage/files.js';

/**
 * Direction for graph traversal.
 */
export type TraversalDirection = 'upstream' | 'downstream' | 'both';

/**
 * A node in the traversal result with its relationships.
 */
export interface GraphNode {
  node: AnyLatticeNode;
  edges: {
    type: string;
    target: string;
    version: string;
    direction: 'incoming' | 'outgoing';
  }[];
}

/**
 * Drift report for a single node.
 */
export interface DriftReport {
  nodeId: string;
  nodeType: string;
  currentVersion: string;
  driftItems: {
    edgeType: string;
    targetId: string;
    boundVersion: string;
    currentVersion: string;
    severity: 'patch' | 'minor' | 'major';
  }[];
}

/**
 * Build an index of all nodes by ID.
 */
export function buildNodeIndex(latticeRoot: string): Map<string, AnyLatticeNode> {
  const index = new Map<string, AnyLatticeNode>();

  const types = ['sources', 'theses', 'requirements', 'implementations'] as const;
  for (const type of types) {
    const nodes = loadNodesByType(latticeRoot, type);
    for (const node of nodes) {
      index.set(node.id, node);
    }
  }

  return index;
}

/**
 * Extract all edge references from a node.
 */
function getEdgeReferences(node: AnyLatticeNode): EdgeReference[] {
  const edges: EdgeReference[] = [];

  if ('edges' in node && node.edges) {
    const nodeEdges = node.edges as Record<string, EdgeReference[] | undefined>;
    for (const refs of Object.values(nodeEdges)) {
      if (Array.isArray(refs)) {
        edges.push(...refs);
      }
    }
  }

  return edges;
}

/**
 * Traverse the graph from a starting node.
 */
export function traverseGraph(
  startId: string,
  nodeIndex: Map<string, AnyLatticeNode>,
  direction: TraversalDirection = 'both',
  maxDepth: number = 3
): GraphNode[] {
  const visited = new Set<string>();
  const result: GraphNode[] = [];

  function traverse(nodeId: string, depth: number) {
    if (depth > maxDepth || visited.has(nodeId)) return;
    visited.add(nodeId);

    const node = nodeIndex.get(nodeId);
    if (!node) return;

    const graphNode: GraphNode = { node, edges: [] };

    // Outgoing edges (from this node)
    if (direction === 'downstream' || direction === 'both') {
      const refs = getEdgeReferences(node);
      for (const ref of refs) {
        graphNode.edges.push({
          type: 'outgoing',
          target: ref.target,
          version: ref.version,
          direction: 'outgoing',
        });
        traverse(ref.target, depth + 1);
      }
    }

    // Incoming edges (to this node)
    if (direction === 'upstream' || direction === 'both') {
      for (const [otherId, otherNode] of nodeIndex) {
        if (otherId === nodeId) continue;
        const refs = getEdgeReferences(otherNode);
        for (const ref of refs) {
          if (ref.target === nodeId) {
            graphNode.edges.push({
              type: 'incoming',
              target: otherId,
              version: ref.version,
              direction: 'incoming',
            });
            traverse(otherId, depth + 1);
          }
        }
      }
    }

    result.push(graphNode);
  }

  traverse(startId, 0);
  return result;
}

/**
 * Compare semver versions and return severity of change.
 */
function compareVersions(
  oldVersion: string | undefined,
  newVersion: string | undefined
): 'patch' | 'minor' | 'major' | 'none' {
  if (!oldVersion || !newVersion) return 'none';

  const [oldMajor = 0, oldMinor = 0, oldPatch = 0] = oldVersion
    .split('.')
    .map(Number);
  const [newMajor = 0, newMinor = 0, newPatch = 0] = newVersion
    .split('.')
    .map(Number);

  if (newMajor > oldMajor) return 'major';
  if (newMinor > oldMinor) return 'minor';
  if (newPatch > oldPatch) return 'patch';
  return 'none';
}

/**
 * Find all drift in the lattice.
 */
export function findDrift(latticeRoot: string): DriftReport[] {
  const nodeIndex = buildNodeIndex(latticeRoot);
  const reports: DriftReport[] = [];

  for (const [nodeId, node] of nodeIndex) {
    const refs = getEdgeReferences(node);
    const driftItems: DriftReport['driftItems'] = [];

    for (const ref of refs) {
      const targetNode = nodeIndex.get(ref.target);
      if (!targetNode) continue;

      const severity = compareVersions(ref.version, targetNode.version);
      if (severity !== 'none') {
        driftItems.push({
          edgeType: 'edge',
          targetId: ref.target,
          boundVersion: ref.version,
          currentVersion: targetNode.version,
          severity,
        });
      }
    }

    if (driftItems.length > 0) {
      reports.push({
        nodeId,
        nodeType: node.type,
        currentVersion: node.version,
        driftItems,
      });
    }
  }

  return reports;
}
