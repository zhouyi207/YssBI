import { describe, expect, it } from 'vitest';
import type { NodeData, PinData } from '@/shared/types/store/graph';
import { toUiNode, uiNodeHasNoHeader } from './nodeView';

const baseNode: NodeData = {
  id: 'node-1',
  graphPath: 'graph-1',
  nodeType: 'math.add',
  category: [],
  title: 'Projected Add',
  inputs: ['pin-in'],
  outputs: ['pin-out'],
  position: { x: 10, y: 20 },
  display: {
    title: 'Projected Add',
    description: 'Projected description',
    userLabel: null,
    iconId: null,
    styleId: 'math',
  },
};

const inputPin: PinData = {
  id: 'pin-in',
  nodeId: 'node-1',
  name: 'A',
  type: 'object',
  direction: 'input',
};

const outputPin: PinData = {
  id: 'pin-out',
  nodeId: 'node-1',
  name: 'Result',
  type: 'object',
  direction: 'output',
};

describe('toUiNode', () => {
  it('maps projected display and pin slices to a canvas node', () => {
    const view = toUiNode(baseNode, {
      pins: [
        { pin: inputPin, connectionIds: ['connection-1'] },
        { pin: outputPin, connectionIds: ['connection-1'] },
      ],
    });

    expect(view).toMatchObject({
      id: 'node-1',
      nodeType: 'math.add',
      title: 'Projected Add',
      description: 'Projected description',
      display: baseNode.display,
      parameterEditors: [],
      diagnostics: [],
      uiStyle: 'math',
    });
    expect(view.inputs[0].connected).toBe(true);
    expect(view.outputs[0].connectionIds).toEqual(['connection-1']);
  });

});

describe('uiNodeHasNoHeader', () => {
  it('returns true for math style nodes', () => {
    expect(uiNodeHasNoHeader({ uiStyle: 'math' })).toBe(true);
    expect(uiNodeHasNoHeader({ uiStyle: 'default' })).toBe(false);
  });
});
