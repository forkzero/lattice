/**
 * File-based storage layer for Lattice.
 *
 * Linked requirements: REQ-CORE-004
 */

import { readFileSync, writeFileSync, existsSync, readdirSync } from 'fs';
import { join, dirname } from 'path';
import { parse, stringify } from 'yaml';
import type { AnyLatticeNode } from '../core/types.js';

/**
 * Default lattice directory name.
 */
export const LATTICE_DIR = '.lattice';

/**
 * Find the lattice root directory by walking up from cwd.
 */
export function findLatticeRoot(startDir: string = process.cwd()): string | null {
  let dir = startDir;
  while (dir !== dirname(dir)) {
    if (existsSync(join(dir, LATTICE_DIR))) {
      return dir;
    }
    dir = dirname(dir);
  }
  return null;
}

/**
 * Load a single node from a YAML file.
 */
export function loadNode(filePath: string): AnyLatticeNode {
  const content = readFileSync(filePath, 'utf-8');
  return parse(content) as AnyLatticeNode;
}

/**
 * Save a node to a YAML file.
 */
export function saveNode(filePath: string, node: AnyLatticeNode): void {
  const content = stringify(node, { lineWidth: 0 });
  writeFileSync(filePath, content, 'utf-8');
}

/**
 * Load all nodes from a directory recursively.
 */
export function loadAllNodes(dir: string): AnyLatticeNode[] {
  const nodes: AnyLatticeNode[] = [];

  function walkDir(currentDir: string) {
    const entries = readdirSync(currentDir, { withFileTypes: true });
    for (const entry of entries) {
      const fullPath = join(currentDir, entry.name);
      if (entry.isDirectory()) {
        walkDir(fullPath);
      } else if (entry.name.endsWith('.yaml') || entry.name.endsWith('.yml')) {
        try {
          nodes.push(loadNode(fullPath));
        } catch {
          // Skip invalid files
        }
      }
    }
  }

  if (existsSync(dir)) {
    walkDir(dir);
  }

  return nodes;
}

/**
 * Load all nodes of a specific type.
 */
export function loadNodesByType(
  latticeRoot: string,
  type: 'sources' | 'theses' | 'requirements' | 'implementations'
): AnyLatticeNode[] {
  const dir = join(latticeRoot, LATTICE_DIR, type);
  return loadAllNodes(dir);
}
