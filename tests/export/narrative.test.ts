/**
 * Tests for narrative export.
 *
 * Linked requirements: REQ-CORE-009
 */

import { describe, it, expect } from 'vitest';
import { exportNarrative } from '../../src/export/narrative.js';
import type {
  SourceNode,
  ThesisNode,
  RequirementNode,
  ImplementationNode,
} from '../../src/core/types.js';

const mockSource: SourceNode = {
  id: 'SRC-TEST-001',
  type: 'source',
  title: 'Test Research',
  body: 'Research about testing.',
  status: 'active',
  version: '1.0.0',
  created_at: '2026-02-03T00:00:00Z',
  created_by: 'human:test',
  meta: {
    citations: ['Test et al., 2026'],
    reliability: 'peer_reviewed',
    retrieved_at: '2026-02-03',
  },
};

const mockThesis: ThesisNode = {
  id: 'THX-TEST-001',
  type: 'thesis',
  title: 'Testing is Important',
  body: 'Tests improve software quality.',
  status: 'active',
  version: '1.0.0',
  created_at: '2026-02-03T00:00:00Z',
  created_by: 'human:test',
  meta: {
    category: 'technical',
    confidence: 0.9,
  },
  edges: {
    supported_by: [{ target: 'SRC-TEST-001', version: '1.0.0' }],
  },
};

const mockRequirement: RequirementNode = {
  id: 'REQ-TEST-001',
  type: 'requirement',
  title: 'Automated Testing',
  body: 'The system should have automated tests.',
  status: 'active',
  version: '1.0.0',
  created_at: '2026-02-03T00:00:00Z',
  created_by: 'human:test',
  priority: 'P0',
  category: 'TEST',
  edges: {
    derives_from: [{ target: 'THX-TEST-001', version: '1.0.0' }],
  },
};

const mockImplementation: ImplementationNode = {
  id: 'IMP-TEST-001',
  type: 'implementation',
  title: 'Test Suite',
  body: 'Vitest test suite.',
  status: 'active',
  version: '1.0.0',
  created_at: '2026-02-03T00:00:00Z',
  created_by: 'human:test',
  meta: {
    language: 'typescript',
    files: [{ path: 'tests/example.test.ts' }],
  },
  edges: {
    satisfies: [{ target: 'REQ-TEST-001', version: '1.0.0' }],
  },
};

describe('exportNarrative', () => {
  const mockData = {
    sources: [mockSource],
    theses: [mockThesis],
    requirements: [mockRequirement],
    implementations: [mockImplementation],
  };

  describe('investor audience', () => {
    it('should generate markdown with theses section', () => {
      const output = exportNarrative(mockData, {
        audience: 'investor',
        title: 'Test Project',
      });

      expect(output).toContain('# Test Project');
      expect(output).toContain('## Strategic Thesis');
      expect(output).toContain('Testing is Important');
    });

    it('should include research support', () => {
      const output = exportNarrative(mockData, {
        audience: 'investor',
      });

      expect(output).toContain('**Research Support:**');
      expect(output).toContain('Test Research');
    });

    it('should show progress', () => {
      const output = exportNarrative(mockData, {
        audience: 'investor',
      });

      expect(output).toContain('## Progress');
      expect(output).toContain('1 of 1 requirements implemented (100%)');
    });
  });

  describe('contributor audience', () => {
    it('should list open requirements', () => {
      const dataWithOpenReq = {
        ...mockData,
        implementations: [], // No implementations
      };

      const output = exportNarrative(dataWithOpenReq, {
        audience: 'contributor',
        title: 'Test Project',
      });

      expect(output).toContain('# Contributing to Test Project');
      expect(output).toContain('## Open Requirements');
      expect(output).toContain('REQ-TEST-001');
    });

    it('should show completed requirements', () => {
      const output = exportNarrative(mockData, {
        audience: 'contributor',
      });

      expect(output).toContain('## Completed');
      expect(output).toContain('✅ REQ-TEST-001');
    });
  });

  describe('overview audience', () => {
    it('should show summary statistics', () => {
      const output = exportNarrative(mockData, {
        audience: 'overview',
        title: 'Test Project',
      });

      expect(output).toContain('# Test Project Overview');
      expect(output).toContain('## Why');
      expect(output).toContain('## What');
      expect(output).toContain('## Progress');
    });
  });

  describe('visibility filtering', () => {
    it('should exclude internal nodes by default', () => {
      const dataWithInternal = {
        ...mockData,
        theses: [
          mockThesis,
          {
            ...mockThesis,
            id: 'THX-INTERNAL',
            title: 'Internal Thesis',
            visibility: 'internal',
          } as ThesisNode & { visibility: string },
        ],
      };

      const output = exportNarrative(dataWithInternal, {
        audience: 'investor',
      });

      expect(output).toContain('Testing is Important');
      expect(output).not.toContain('Internal Thesis');
    });

    it('should include internal nodes when flag is set', () => {
      const dataWithInternal = {
        ...mockData,
        theses: [
          mockThesis,
          {
            ...mockThesis,
            id: 'THX-INTERNAL',
            title: 'Internal Thesis',
            visibility: 'internal',
          } as ThesisNode & { visibility: string },
        ],
      };

      const output = exportNarrative(dataWithInternal, {
        audience: 'investor',
        includeInternal: true,
      });

      expect(output).toContain('Testing is Important');
      expect(output).toContain('Internal Thesis');
    });
  });
});
