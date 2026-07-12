import { beforeEach, describe, expect, it } from 'vitest';
import type { NodeData, PinData } from '@/shared/types/store/graph';
import type { NodeDefinition } from '@/shared/types/domain/node';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { toUiNode, uiNodeHasNoHeader } from './nodeView';

const baseNode: NodeData = {
  id: 'node-1',
  graphPath: 'graph-1',
  nodeType: 'Math:Add',
  category: ['Math'],
  title: 'Add',
  inputs: ['pin-in'],
  outputs: ['pin-out'],
  position: { x: 10, y: 20 },
};

const inputPin: PinData = {
  id: 'pin-in',
  nodeId: 'node-1',
  name: 'A',
  type: 'object',
  direction: 'input',
  dataType: { kind: 'Float64' },
};

const outputPin: PinData = {
  id: 'pin-out',
  nodeId: 'node-1',
  name: 'Result',
  type: 'object',
  direction: 'output',
  dataType: { kind: 'Float64' },
};

describe('toUiNode', () => {
  beforeEach(() => {
    const def: NodeDefinition = {
      name: 'Add',
      category: ['Math'],
      nodeType: 'Math:Add',
      nodeMetadata: {
        uiStyle: 'math',
        supports_dynamic_pins: false,
        graph_scope: 'any',
        shell_role: null,
      },
      pinSlots: [],
      typeCapabilities: [],
    };
    useNodeRegistryStore.getState().setDefinitionsFromSchema(
      new Map([['Math:Add', def]]),
    );
  });

  it('maps NodeData and pin slices to UINode with connection views', () => {
    const view = toUiNode(baseNode, {
      pins: [
        { pin: inputPin, connectionIds: ['pin-out->pin-in'] },
        { pin: outputPin, connectionIds: ['pin-out->pin-in'] },
      ],
    });

    expect(view.id).toBe('node-1');
    expect(view.uiStyle).toBe('math');
    expect(view.inputs).toHaveLength(1);
    expect(view.outputs).toHaveLength(1);
    expect(view.inputs[0].connected).toBe(true);
    expect(view.outputs[0].connectionIds).toEqual(['pin-out->pin-in']);
  });

  it('applies title override for contextual nodes', () => {
    const view = toUiNode(baseNode, {
      title: 'My Function',
      pins: [],
    });
    expect(view.title).toBe('My Function');
  });
});

describe('uiNodeHasNoHeader', () => {
  it('returns true for math style nodes', () => {
    expect(uiNodeHasNoHeader({ uiStyle: 'math' })).toBe(true);
    expect(uiNodeHasNoHeader({ uiStyle: 'default' })).toBe(false);
  });
});
