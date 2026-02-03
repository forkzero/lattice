/**
 * Tests for core type definitions.
 *
 * Linked requirements: REQ-CORE-001, REQ-CORE-002
 */

import { describe, it, expect } from 'vitest';
import type {
  NodeType,
  EdgeType,
  LatticeNode,
  RequirementNode,
  SourceNode,
} from '../../src/core/types.js';

describe('NodeType', () => {
  it('should support four primary types', () => {
    const types: NodeType[] = ['source', 'thesis', 'requirement', 'implementation'];
    expect(types).toHaveLength(4);
  });
});

describe('EdgeType', () => {
  it('should support justification edges', () => {
    const justificationEdges: EdgeType[] = ['supports', 'derives', 'satisfies'];
    expect(justificationEdges).toHaveLength(3);
  });

  it('should support dependency edges', () => {
    const dependencyEdges: EdgeType[] = ['depends_on', 'extends'];
    expect(dependencyEdges).toHaveLength(2);
  });

  it('should support feedback edges', () => {
    const feedbackEdges: EdgeType[] = ['reveals_gap_in', 'challenges', 'validates'];
    expect(feedbackEdges).toHaveLength(3);
  });

  it('should support conflict edges', () => {
    const conflictEdges: EdgeType[] = ['conflicts_with', 'supersedes'];
    expect(conflictEdges).toHaveLength(2);
  });
});

describe('LatticeNode', () => {
  it('should have required fields', () => {
    const node: LatticeNode = {
      id: 'TEST-001',
      type: 'requirement',
      title: 'Test Requirement',
      body: 'This is a test requirement.',
      status: 'active',
      version: '1.0.0',
      created_at: '2026-02-03T00:00:00Z',
      created_by: 'human:test',
    };

    expect(node.id).toBe('TEST-001');
    expect(node.type).toBe('requirement');
    expect(node.title).toBe('Test Requirement');
    expect(node.body).toContain('test requirement');
    expect(node.status).toBe('active');
    expect(node.version).toBe('1.0.0');
    expect(node.created_by).toBe('human:test');
  });
});

describe('SourceNode', () => {
  it('should have source-specific metadata', () => {
    const source: SourceNode = {
      id: 'SRC-TEST-001',
      type: 'source',
      title: 'Test Research Paper',
      body: 'Abstract of the paper.',
      status: 'active',
      version: '1.0.0',
      created_at: '2026-02-03T00:00:00Z',
      created_by: 'human:test',
      meta: {
        url: 'https://example.com/paper',
        citations: ['Author et al., 2024'],
        reliability: 'peer_reviewed',
        retrieved_at: '2026-02-03',
      },
    };

    expect(source.meta.reliability).toBe('peer_reviewed');
    expect(source.meta.url).toBe('https://example.com/paper');
  });
});

describe('RequirementNode', () => {
  it('should have requirement-specific fields', () => {
    const requirement: RequirementNode = {
      id: 'REQ-TEST-001',
      type: 'requirement',
      title: 'Test Feature',
      body: 'The system should do something.',
      status: 'active',
      version: '1.0.0',
      created_at: '2026-02-03T00:00:00Z',
      created_by: 'human:test',
      priority: 'P0',
      category: 'TEST',
      tags: ['test', 'example'],
      acceptance: [
        {
          id: 'AT-001',
          given: 'some precondition',
          when: 'some action',
          then: 'some result',
          verification: 'automated',
        },
      ],
      edges: {
        derives_from: [
          {
            target: 'THX-TEST-001',
            version: '1.0.0',
            rationale: 'Test derivation',
          },
        ],
      },
    };

    expect(requirement.priority).toBe('P0');
    expect(requirement.category).toBe('TEST');
    expect(requirement.acceptance).toHaveLength(1);
    expect(requirement.edges.derives_from).toHaveLength(1);
  });
});
