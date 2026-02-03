/**
 * Tests for file-based storage.
 *
 * Linked requirements: REQ-CORE-004
 */

import { describe, it, expect } from 'vitest';
import { join } from 'path';
import { loadNode, loadAllNodes, findLatticeRoot } from '../../src/storage/files.js';

describe('findLatticeRoot', () => {
  it('should find lattice root from current directory', () => {
    const root = findLatticeRoot(process.cwd());
    // We're in the lattice repo which has .lattice/
    expect(root).toBe(process.cwd());
  });
});

describe('loadNode', () => {
  it('should load a source node from YAML', () => {
    const filePath = join(
      process.cwd(),
      '.lattice/sources/agent-code-generation.yaml'
    );
    const node = loadNode(filePath);

    expect(node.id).toBe('SRC-AGENT-CODE-GEN');
    expect(node.type).toBe('source');
    expect(node.title).toContain('AI Agents');
    expect(node.status).toBe('active');
  });

  it('should load a thesis node from YAML', () => {
    const filePath = join(
      process.cwd(),
      '.lattice/theses/requirements-as-durable-artifact.yaml'
    );
    const node = loadNode(filePath);

    expect(node.id).toBe('THX-REQUIREMENTS-DURABLE');
    expect(node.type).toBe('thesis');
  });

  it('should load a requirement node from YAML', () => {
    const filePath = join(
      process.cwd(),
      '.lattice/requirements/core/001-node-types.yaml'
    );
    const node = loadNode(filePath);

    expect(node.id).toBe('REQ-CORE-001');
    expect(node.type).toBe('requirement');
    expect((node as any).priority).toBe('P0');
  });
});

describe('loadAllNodes', () => {
  it('should load all nodes from sources directory', () => {
    const sourcesDir = join(process.cwd(), '.lattice/sources');
    const nodes = loadAllNodes(sourcesDir);

    expect(nodes.length).toBeGreaterThan(0);
    for (const node of nodes) {
      expect(node.type).toBe('source');
    }
  });

  it('should load all nodes from requirements directory recursively', () => {
    const requirementsDir = join(process.cwd(), '.lattice/requirements');
    const nodes = loadAllNodes(requirementsDir);

    expect(nodes.length).toBeGreaterThan(0);
    for (const node of nodes) {
      expect(node.type).toBe('requirement');
    }
  });

  it('should return empty array for non-existent directory', () => {
    const nodes = loadAllNodes('/nonexistent/path');
    expect(nodes).toEqual([]);
  });
});
