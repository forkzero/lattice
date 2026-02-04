/**
 * Add nodes to the lattice.
 *
 * Linked requirements: REQ-CLI-002
 */

import { writeFileSync, existsSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { stringify } from 'yaml';
import type {
  RequirementNode,
  ThesisNode,
  SourceNode,
  Priority,
  EdgeReference,
} from '../core/types.js';
import { LATTICE_DIR } from './files.js';

export interface AddRequirementOptions {
  id: string;
  title: string;
  body: string;
  priority: Priority;
  category: string;
  tags?: string[];
  derives_from?: string[];
  depends_on?: string[];
  status?: 'draft' | 'active';
  created_by?: string;
}

export interface AddThesisOptions {
  id: string;
  title: string;
  body: string;
  category: 'value_prop' | 'market' | 'technical' | 'risk' | 'competitive';
  confidence?: number;
  supported_by?: string[];
  status?: 'draft' | 'active';
  created_by?: string;
}

export interface AddSourceOptions {
  id: string;
  title: string;
  body: string;
  url?: string;
  citations?: string[];
  reliability?: 'peer_reviewed' | 'industry' | 'blog' | 'unverified';
  status?: 'draft' | 'active';
  created_by?: string;
}

function ensureDir(filePath: string): void {
  const dir = dirname(filePath);
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }
}

function makeEdgeRefs(targets: string[] | undefined): EdgeReference[] | undefined {
  if (!targets || targets.length === 0) return undefined;
  return targets.map((target) => ({
    target,
    version: '1.0.0', // Default to 1.0.0, will be updated on verify
  }));
}

/**
 * Add a requirement to the lattice.
 */
export function addRequirement(
  latticeRoot: string,
  options: AddRequirementOptions
): string {
  const now = new Date().toISOString();

  const node: RequirementNode = {
    id: options.id,
    type: 'requirement',
    title: options.title,
    body: options.body,
    priority: options.priority,
    category: options.category,
    tags: options.tags,
    status: options.status || 'active',
    version: '1.0.0',
    created_at: now,
    created_by: options.created_by || 'unknown',
    edges: {},
  };

  if (options.derives_from) {
    node.edges.derives_from = makeEdgeRefs(options.derives_from);
  }
  if (options.depends_on) {
    node.edges.depends_on = makeEdgeRefs(options.depends_on);
  }

  // Determine file path based on category
  const categoryDir = options.category.toLowerCase();
  const idNumber = options.id.split('-').pop() || '000';
  const slug = options.title.toLowerCase().replace(/[^a-z0-9]+/g, '-').substring(0, 40);
  const fileName = `${idNumber}-${slug}.yaml`;
  const filePath = join(
    latticeRoot,
    LATTICE_DIR,
    'requirements',
    categoryDir,
    fileName
  );

  ensureDir(filePath);
  const content = stringify(node, { lineWidth: 0 });
  writeFileSync(filePath, content, 'utf-8');

  return filePath;
}

/**
 * Add a thesis to the lattice.
 */
export function addThesis(
  latticeRoot: string,
  options: AddThesisOptions
): string {
  const now = new Date().toISOString();

  const node: ThesisNode = {
    id: options.id,
    type: 'thesis',
    title: options.title,
    body: options.body,
    status: options.status || 'active',
    version: '1.0.0',
    created_at: now,
    created_by: options.created_by || 'unknown',
    meta: {
      category: options.category,
      confidence: options.confidence || 0.8,
    },
    edges: {},
  };

  if (options.supported_by) {
    node.edges.supported_by = makeEdgeRefs(options.supported_by);
  }

  const slug = options.id.toLowerCase().replace(/^thx-/, '');
  const fileName = `${slug}.yaml`;
  const filePath = join(latticeRoot, LATTICE_DIR, 'theses', fileName);

  ensureDir(filePath);
  const content = stringify(node, { lineWidth: 0 });
  writeFileSync(filePath, content, 'utf-8');

  return filePath;
}

/**
 * Add a source to the lattice.
 */
export function addSource(
  latticeRoot: string,
  options: AddSourceOptions
): string {
  const now = new Date().toISOString();

  const node: SourceNode = {
    id: options.id,
    type: 'source',
    title: options.title,
    body: options.body,
    status: options.status || 'active',
    version: '1.0.0',
    created_at: now,
    created_by: options.created_by || 'unknown',
    meta: {
      url: options.url,
      citations: options.citations,
      reliability: options.reliability || 'unverified',
      retrieved_at: now.split('T')[0],
    },
  };

  const slug = options.id.toLowerCase().replace(/^src-/, '');
  const fileName = `${slug}.yaml`;
  const filePath = join(latticeRoot, LATTICE_DIR, 'sources', fileName);

  ensureDir(filePath);
  const content = stringify(node, { lineWidth: 0 });
  writeFileSync(filePath, content, 'utf-8');

  return filePath;
}
