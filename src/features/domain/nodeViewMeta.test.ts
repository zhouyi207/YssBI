import { beforeEach, describe, expect, it } from 'vitest';
import type { NodeDefinition } from '@/shared/types/domain/node';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { resolveNodeViewMeta } from './nodeViewMeta';

function defaultNodeMetadata(uiStyle: string) {
  return {
    uiStyle,
    supports_dynamic_pins: false,
    graph_scope: 'any' as const,
    shell_role: null,
  };
}

function mathAddDefinition(): NodeDefinition {
  return {
    name: 'Add',
    category: ['Math'],
    nodeType: 'Math:Add',
    nodeMetadata: defaultNodeMetadata('math'),
    pinSlots: [],
    typeCapabilities: [],
  };
}

describe('resolveNodeViewMeta', () => {
  beforeEach(() => {
    useNodeRegistryStore.getState().setDefinitionsFromSchema(
      new Map([['Math:Add', mathAddDefinition()]]),
    );
  });

  it('derives uiStyle from catalog, ignoring stale instance snapshots', () => {
    const meta = resolveNodeViewMeta({
      nodeType: 'Math:Add',
      title: 'Add',
    });
    expect(meta.uiStyle).toBe('math');
  });

  it('falls back to default when node type is unknown', () => {
    const meta = resolveNodeViewMeta({ nodeType: 'Unknown:Node' });
    expect(meta.uiStyle).toBe('default');
  });
});
