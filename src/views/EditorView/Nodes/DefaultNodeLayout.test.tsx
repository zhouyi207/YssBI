// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useDatabaseStore, useVariableStore } from '@/features/core/dataStore';
import type { UINode } from '@/shared/types/ui';
import { DefaultNodeLayout } from './DefaultNodeLayout';

vi.mock('../Pins/Pin', () => ({
  Pin: ({ name }: { name: string }) => <span data-testid="pin-name">{name}</span>,
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

function resourceNode(nodeType: string, resource: 'variable' | 'database'): UINode {
  return {
    id: `${resource}-node`,
    graphPath: 'events/Main.yssbi-event',
    nodeType,
    category: [],
    title: 'Localized resource node',
    uiStyle: 'default',
    position: { x: 0, y: 0 },
    variableId: resource === 'variable' ? 'variable-1' : undefined,
    dataframeId: resource === 'database' ? 'database-1' : undefined,
    inputs: [],
    outputs: [{
      id: 'resource-output',
      nodeId: `${resource}-node`,
      name: 'Projected resource',
      type: 'object',
      direction: 'output',
      kind: 'data',
      dataType: { kind: 'Float64' },
      connected: false,
      linkCount: 0,
      connectionIds: [],
    }],
  } as UINode;
}

describe('DefaultNodeLayout resource pin projection', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    useVariableStore.setState({
      variables: {
        'variable-1': {
          id: 'variable-1',
          name: 'Counter',
          dataType: { kind: 'Float64' },
          dataValue: { kind: 'Float64', value: 1 },
          description: '',
          scope: { type: 'global' },
          tags: [],
        },
      },
    });
    useDatabaseStore.setState({
      databases: {
        'database-1': {
          id: 'database-1',
          name: 'Sales',
          engine: { inMemory: { name: 'sales' } },
          schemaVersion: 1,
          required: false,
        },
      },
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    useVariableStore.setState({ variables: {} });
    useDatabaseStore.setState({ databases: {} });
  });

  it.each([
    ['yssbi.project.variable.get', 'Variables:Get Variable', 'variable', 'Counter'],
    ['yssbi.project.variable.set', 'Variables:Set Variable', 'variable', 'Counter'],
    ['yssbi.dataframe.source.get', 'Data:Get DataFrame', 'database', 'Sales'],
  ] as const)('uses stable %s identity but ignores legacy %s', (stableId, legacyId, resource, expectedName) => {
    act(() => root.render(<DefaultNodeLayout node={resourceNode(stableId, resource)} />));
    expect(container.querySelector('[data-testid="pin-name"]')?.textContent).toBe(expectedName);

    act(() => root.render(<DefaultNodeLayout node={resourceNode(legacyId, resource)} />));
    expect(container.querySelector('[data-testid="pin-name"]')?.textContent).toBe('Projected resource');
  });
});
