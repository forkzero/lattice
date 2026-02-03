/**
 * Lattice - A knowledge coordination protocol for human-agent collaboration
 *
 * @packageDocumentation
 */

export type { LatticeNode, NodeType, EdgeType } from './core/types.js';
export { loadNode, saveNode, loadAllNodes } from './storage/files.js';
export { traverseGraph, findDrift } from './graph/traverse.js';
