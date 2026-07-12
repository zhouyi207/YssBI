import { describe, expect, it } from 'vitest';
import { createDataSignaturePin } from '@/shared/types/domain/functionSignaturePin';
import type { Pin } from '@/shared/types/domain/pin';
import { CALL_FUNCTION_NODE_TYPE } from '@/features/domain/nodeDefinition';
import { buildContextualCatalogItems } from './buildContextualCatalogItems';

function draggedPin(partial: Partial<Pin> & Pick<Pin, 'direction' | 'type' | 'dataType'>): Pin {
  return {
    id: partial.id ?? 'in-1',
    nodeId: partial.nodeId ?? 'n1',
    name: partial.name ?? 'pin',
    ...partial,
  } as Pin;
}

const callRegistryDef = {
  name: 'Call Function',
  category: ['Functions'],
  nodeType: CALL_FUNCTION_NODE_TYPE,
  nodeMetadata: {
    uiStyle: 'function',
    supports_dynamic_pins: true,
    graph_scope: 'any' as const,
    shell_role: null,
  },
  pinSlots: [],
  typeCapabilities: [],
};

describe('buildContextualCatalogItems', () => {
  it('excludes Call Function when signature dataType is incompatible', () => {
    const filterPin = draggedPin({
      direction: 'input',
      type: 'object',
      dataType: { kind: 'Float64' },
    });

    const items = buildContextualCatalogItems({
      definitions: [callRegistryDef],
      filterPin,
      functions: {
        'fn-1': {
          id: 'fn-1',
          name: 'New Function',
          functionInputs: [createDataSignaturePin('pin-c3f93066', '新 Pin', { kind: 'Int64' })],
          functionOutputs: [],
        },
      },
      graphKind: 'event',
    });

    expect(items.some((i) => i.nodeType === CALL_FUNCTION_NODE_TYPE)).toBe(false);
  });

  it('includes Call Function when signature pin type is compatible', () => {
    const filterPin = draggedPin({
      direction: 'output',
      type: 'object',
      dataType: { kind: 'Float64' },
    });

    const items = buildContextualCatalogItems({
      definitions: [callRegistryDef],
      filterPin,
      functions: {
        'fn-1': {
          id: 'fn-1',
          name: 'My Func',
          functionInputs: [createDataSignaturePin('sig-1', 'Value', { kind: 'Float64' })],
          functionOutputs: [],
        },
      },
      graphKind: 'event',
    });

    expect(items).toEqual([
      expect.objectContaining({
        nodeType: CALL_FUNCTION_NODE_TYPE,
        title: 'My Func',
        overrides: { subGraphPath: 'fn-1', title: 'My Func' },
      }),
    ]);
  });
});
