#!/usr/bin/env node
/**
 * Lattice CLI - Command-line interface for Lattice operations.
 *
 * Linked requirements: REQ-CLI-001 through REQ-CLI-005, REQ-CORE-009
 */

import { Command } from 'commander';
import chalk from 'chalk';
import { findLatticeRoot, loadNodesByType } from '../storage/files.js';
import { buildNodeIndex, findDrift } from '../graph/traverse.js';
import { exportNarrative, type Audience } from '../export/narrative.js';
import type {
  SourceNode,
  ThesisNode,
  RequirementNode,
  ImplementationNode,
} from '../core/types.js';

const program = new Command();

program
  .name('lattice')
  .description('A knowledge coordination protocol for human-agent collaboration')
  .version('0.1.0');

// Init command
program
  .command('init')
  .description('Initialize a new lattice in the current directory')
  .option('-f, --force', 'Overwrite existing lattice')
  .action((_options) => {
    console.log(chalk.yellow('lattice init not yet implemented'));
    console.log('Would create .lattice/ directory structure');
  });

// List command
program
  .command('list <type>')
  .description('List nodes of a given type (sources, theses, requirements, implementations)')
  .option('-s, --status <status>', 'Filter by status')
  .option('-p, --priority <priority>', 'Filter by priority (requirements only)')
  .action((type, _options) => {
    const root = findLatticeRoot();
    if (!root) {
      console.error(chalk.red('Not in a lattice directory'));
      process.exit(1);
    }

    const nodeIndex = buildNodeIndex(root);
    const typeMap: Record<string, string> = {
      sources: 'source',
      theses: 'thesis',
      requirements: 'requirement',
      implementations: 'implementation',
    };

    const targetType = typeMap[type];
    if (!targetType) {
      console.error(chalk.red(`Unknown type: ${type}`));
      process.exit(1);
    }

    for (const [id, node] of nodeIndex) {
      if (node.type === targetType) {
        console.log(`${chalk.cyan(id)} - ${node.title}`);
      }
    }
  });

// Drift command
program
  .command('drift')
  .description('Check for version drift in the lattice')
  .option('--check', 'Exit with non-zero status if drift detected')
  .action((options) => {
    const root = findLatticeRoot();
    if (!root) {
      console.error(chalk.red('Not in a lattice directory'));
      process.exit(1);
    }

    const reports = findDrift(root);

    if (reports.length === 0) {
      console.log(chalk.green('No drift detected'));
      return;
    }

    console.log(chalk.yellow(`DRIFT DETECTED (${reports.length} nodes):\n`));

    for (const report of reports) {
      console.log(chalk.cyan(`${report.nodeId} (${report.nodeType})`));
      for (const item of report.driftItems) {
        const severityColor =
          item.severity === 'major'
            ? chalk.red
            : item.severity === 'minor'
              ? chalk.yellow
              : chalk.gray;
        console.log(
          `  → ${item.targetId}: ${item.boundVersion} → ${item.currentVersion} ${severityColor(`[${item.severity}]`)}`
        );
      }
      console.log();
    }

    if (options.check) {
      process.exit(1);
    }
  });

// Get command
program
  .command('get <id>')
  .description('Get a specific node by ID')
  .action((id) => {
    const root = findLatticeRoot();
    if (!root) {
      console.error(chalk.red('Not in a lattice directory'));
      process.exit(1);
    }

    const nodeIndex = buildNodeIndex(root);
    const node = nodeIndex.get(id);

    if (!node) {
      console.error(chalk.red(`Node not found: ${id}`));
      process.exit(1);
    }

    console.log(chalk.cyan(`${node.id} (${node.type})`));
    console.log(chalk.bold(node.title));
    console.log();
    console.log(node.body);
    console.log();
    console.log(chalk.gray(`Status: ${node.status} | Version: ${node.version}`));
  });

// Export command
program
  .command('export')
  .description('Export the lattice to various formats')
  .option(
    '-f, --format <format>',
    'Export format (narrative, json)',
    'narrative'
  )
  .option(
    '-a, --audience <audience>',
    'Target audience for narrative (investor, contributor, overview)',
    'overview'
  )
  .option('-t, --title <title>', 'Document title', 'Lattice')
  .option('--include-internal', 'Include nodes marked as internal')
  .action((options) => {
    const root = findLatticeRoot();
    if (!root) {
      console.error(chalk.red('Not in a lattice directory'));
      process.exit(1);
    }

    if (options.format === 'json') {
      const nodeIndex = buildNodeIndex(root);
      const nodes = Array.from(nodeIndex.values());
      console.log(JSON.stringify(nodes, null, 2));
      return;
    }

    if (options.format === 'narrative') {
      const validAudiences = ['investor', 'contributor', 'overview'];
      if (!validAudiences.includes(options.audience)) {
        console.error(
          chalk.red(
            `Invalid audience: ${options.audience}. Must be one of: ${validAudiences.join(', ')}`
          )
        );
        process.exit(1);
      }

      const sources = loadNodesByType(root, 'sources') as SourceNode[];
      const theses = loadNodesByType(root, 'theses') as ThesisNode[];
      const requirements = loadNodesByType(
        root,
        'requirements'
      ) as RequirementNode[];
      const implementations = loadNodesByType(
        root,
        'implementations'
      ) as ImplementationNode[];

      const output = exportNarrative(
        { sources, theses, requirements, implementations },
        {
          audience: options.audience as Audience,
          title: options.title,
          includeInternal: options.includeInternal || false,
        }
      );

      console.log(output);
      return;
    }

    console.error(chalk.red(`Unknown format: ${options.format}`));
    process.exit(1);
  });

program.parse();
