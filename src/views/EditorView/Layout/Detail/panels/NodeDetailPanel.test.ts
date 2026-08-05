import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { GraphEntityBucket } from '@/features/core/dataStore/graphEntityAccess';
import { selectNodeDetailNode } from './NodeDetailPanel';

function bucket(graphPath: string, title: string): GraphEntityBucket {
  return {
    basis: {
      graphPath,
      graphRevision: 1,
      registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
      resourceVersions: {},
    },
    sourceRevision: 1,
    requestGeneration: 1,
    diagnostics: [],
    hasBlockingDiagnostics: false,
    nodes: {
      shared: {
        id: 'shared',
        graphPath,
        nodeType: 'projected.call-function',
        category: [],
        title,
        inputs: [],
        outputs: [],
        position: { x: 0, y: 0 },
        display: {
          title,
          description: null,
          userLabel: null,
          iconId: null,
          styleId: null,
        },
        parameterEditors: [],
        capabilities: {
          managed: false,
          canCopy: true,
          canDelete: true,
          canEditLabel: false,
          canEditParameters: true,
          hasDynamicPorts: false,
          supportsInlineLiterals: false,
        },
        diagnostics: [],
        subGraphPath: 'legacy/must-not-be-read',
      },
    },
    pins: {},
    connections: {},
    graphNodes: ['shared'],
    nodePins: { shared: [] },
    pinConnections: {},
  };
}

describe('NodeDetailPanel projection selection', () => {
  it('selects an overlapping node id only from the requested graph path', () => {
    const state = {
      graphEntities: {
        first: bucket('first', 'First'),
        second: bucket('second', 'Second'),
      },
    };

    expect(selectNodeDetailNode(state, 'second', 'shared')?.title).toBe('Second');
  });

  it('does not read Call Function legacy fields or legacy catalogs', () => {
    const source = readFileSync(new URL('./NodeDetailPanel.tsx', import.meta.url), 'utf8');

    expect(source).not.toMatch(
      /CALL_FUNCTION_NODE_TYPE|subGraphPath|useFunctionCatalog|useCallFunctionIssue|updateCallFunctionTarget/,
    );
    expect(source).not.toContain('Object.entries(s.graphEntities)');
  });
});
