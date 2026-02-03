/**
 * Core type definitions for Lattice nodes and edges.
 *
 * Linked requirements: REQ-CORE-001, REQ-CORE-002
 */

/**
 * The four primary node types in the lattice.
 */
export type NodeType = 'source' | 'thesis' | 'requirement' | 'implementation';

/**
 * Typed edges with semantic meaning.
 */
export type EdgeType =
  // Justification edges (downward flow)
  | 'supports' // Source → Thesis
  | 'derives' // Thesis → Requirement
  | 'satisfies' // Implementation → Requirement
  // Dependency edges (lateral)
  | 'depends_on' // Requirement → Requirement
  | 'extends' // Requirement → Requirement
  // Feedback edges (upward flow)
  | 'reveals_gap_in' // Implementation → Requirement/Thesis
  | 'challenges' // Any → Thesis
  | 'validates' // Implementation → Thesis
  // Conflict edges
  | 'conflicts_with' // Any → Any
  | 'supersedes'; // Any → Any

/**
 * Node status lifecycle.
 */
export type NodeStatus = 'draft' | 'active' | 'deprecated' | 'superseded';

/**
 * Priority levels for requirements.
 */
export type Priority = 'P0' | 'P1' | 'P2';

/**
 * A version-bound edge reference.
 */
export interface EdgeReference {
  target: string;
  version: string;
  rationale?: string;
  strength?: number;
}

/**
 * An acceptance test for a requirement.
 */
export interface AcceptanceTest {
  id: string;
  given: string;
  when: string;
  then: string;
  verification: 'automated' | 'manual' | 'statistical';
}

/**
 * Base structure for all lattice nodes.
 */
export interface LatticeNode {
  id: string;
  type: NodeType;
  title: string;
  body: string;
  status: NodeStatus;
  version: string;
  created_at: string;
  created_by: string;
  updated_at?: string;
}

/**
 * Source node - primary research artifact.
 */
export interface SourceNode extends LatticeNode {
  type: 'source';
  meta: {
    url?: string;
    citations?: string[];
    reliability: 'peer_reviewed' | 'industry' | 'blog' | 'unverified';
    retrieved_at: string;
  };
}

/**
 * Thesis node - strategic claim.
 */
export interface ThesisNode extends LatticeNode {
  type: 'thesis';
  meta: {
    category: 'value_prop' | 'market' | 'technical' | 'risk' | 'competitive';
    confidence: number;
    valid_until?: string;
  };
  edges: {
    supported_by?: EdgeReference[];
  };
}

/**
 * Requirement node - testable specification.
 */
export interface RequirementNode extends LatticeNode {
  type: 'requirement';
  priority: Priority;
  category: string;
  tags?: string[];
  acceptance?: AcceptanceTest[];
  edges: {
    derives_from?: EdgeReference[];
    depends_on?: EdgeReference[];
    enables?: EdgeReference[];
  };
}

/**
 * Implementation node - code that satisfies requirements.
 */
export interface ImplementationNode extends LatticeNode {
  type: 'implementation';
  meta: {
    language: string;
    files: {
      path: string;
      functions?: string[];
      content_hash?: string;
    }[];
    test_files?: string[];
    coverage?: number;
    last_verified?: string;
    verified_by?: string;
  };
  edges: {
    satisfies?: EdgeReference[];
    reveals_gap_in?: EdgeReference[];
    challenges?: EdgeReference[];
    validates?: EdgeReference[];
  };
}

/**
 * Union type for any lattice node.
 */
export type AnyLatticeNode =
  | SourceNode
  | ThesisNode
  | RequirementNode
  | ImplementationNode;
