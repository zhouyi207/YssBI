import { describe, expect, it } from 'vitest';
import type { NodeData, PinData } from '@/shared/types/store/graph';
import { toUiNode, uiNodeHasNoHeader } from './nodeView';

const baseNode: NodeData = {
  id: 'node-1',
  graphId: 'graph-1',
  nodeType: 'Math:Add',
  category: ['Math'],
  title: 'Add',
  inputs: ['pin-in'],
  outputs: ['pin-out'],
  uiStyle: 'math',
  position: { x: 10, y: 20 },
};

const inputPin: PinData = {
  id: 'pin-in',
  nodeId: 'node-1',
  name: 'A',
  type: 'Float64',
  direction: 'input',
};

const outputPin: PinData = {
  id: 'pin-out',
  nodeId: 'node-1',
  name: 'Result',
  type: 'Float64',
  direction: 'output',
};

describe('toUiNode', () => {
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
