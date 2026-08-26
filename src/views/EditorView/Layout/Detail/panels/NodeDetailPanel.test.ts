// @vitest-environment happy-dom

import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, describe, expect, it, vi } from 'vitest';
import type { GraphEntityBucket } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { NodeDetailPanel, selectNodeDetailNode } from './NodeDetailPanel';
import { NodeInspectPanel } from './NodeInspectPanel';

const katexWarningSpy = vi.hoisted(() => {
  const warn = console.warn.bind(console);
  return vi.spyOn(console, 'warn').mockImplementation((message, ...args) => {
    const quirksWarning = "Warning: KaTeX doesn't work in quirks mode. Make sure your website has a suitable doctype.";
    if (message !== quirksWarning) warn(message, ...args);
  });
});

vi.mock('react-i18next', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-i18next')>()),
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en-US' },
  }),
}));

vi.mock('../node/parameterEditors/NodeParameterEditor', () => ({
  NodeParameterEditor: () => createElement(
    'span',
    { 'data-testid': 'parameter-editor' },
    'parameter editor',
  ),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

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
    outcome: { type: 'success' },
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
        subGraphPath: 'unexpected/must-not-be-read',
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
  afterAll(() => katexWarningSpy.mockRestore());

  it('selects an overlapping node id only from the requested graph path', () => {
    const state = {
      graphEntities: {
        first: bucket('first', 'First'),
        second: bucket('second', 'Second'),
      },
    };

    expect(selectNodeDetailNode(state, 'second', 'shared')?.title).toBe('Second');
  });

  it('renders editable parameters only in NodeInspectPanel', () => {
    const graphPath = 'events/Main.yssbi-event';
    const graphBucket = bucket(graphPath, 'Node');
    graphBucket.nodes.shared.parameterEditors = [{
      key: 'value',
      display: { title: 'Value', description: null },
      editor: 'number',
      presentation: 'inlineAndDetail',
      valueType: { kind: 'Int64' },
      multiline: false,
      value: 42,
      configuration: null,
      inheritedValue: null,
      valueSource: null,
      options: null,
    }];
    useGraphDataStore.setState({ graphEntities: { [graphPath]: graphBucket } });
    const container = document.createElement('div');
    const root = createRoot(container);

    act(() => root.render(createElement(NodeDetailPanel, { graphPath, nodeId: 'shared' })));
    expect(container.querySelector('[data-testid="parameter-editor"]')).toBeNull();

    act(() => root.render(createElement(NodeInspectPanel, { graphPath, nodeId: 'shared' })));
    expect(container.querySelector('[data-testid="parameter-editor"]')).not.toBeNull();

    act(() => root.unmount());
  });
});
