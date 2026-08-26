import { describe, expect, it } from 'vitest';
import type { NodeData, PinData } from '@/shared/types/store/graph';
import {
  REROUTE_NODE_STYLE_ID,
  toUiNode,
  uiNodeIsReroute,
} from './nodeView';

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
      display: baseNode.display,
      parameterEditors: [],
      diagnostics: [],
      uiStyle: 'math',
    });
    expect(view.inputs[0].connected).toBe(true);
    expect(view.outputs[0].connectionIds).toEqual(['connection-1']);
  });

});

describe('uiNodeIsReroute', () => {
  it('classifies only the Rust-authored builtin.reroute style', () => {
    expect(REROUTE_NODE_STYLE_ID).toBe('builtin.reroute');
    expect(uiNodeIsReroute({ uiStyle: 'builtin.reroute' })).toBe(true);
    expect(uiNodeIsReroute({ uiStyle: 'reroute' })).toBe(false);
    expect(uiNodeIsReroute({ uiStyle: 'default' })).toBe(false);
  });

  it('preserves projected reroute position and port descriptors without synthesizing identity', () => {
    const view = toUiNode({
      ...baseNode,
      id: 'reroute-1',
      nodeType: 'opaque.backend.identity',
      position: { x: 135, y: 246 },
      display: { ...baseNode.display!, styleId: 'builtin.reroute' },
    }, {
      pins: [{
        pin: {
          ...inputPin,
          id: 'projected-address-key',
          nodeId: 'reroute-1',
          kind: 'effect',
          address: { kind: 'declared', nodeId: 'reroute-1', portKey: 'input' },
          resolvedType: { display: 'Unknown', resolved: false, dataType: null },
        },
        connectionIds: ['edge-a'],
      }],
    });

    expect(uiNodeIsReroute(view)).toBe(true);
    expect(view.position).toEqual({ x: 135, y: 246 });
    expect(view.nodeType).toBe('opaque.backend.identity');
    expect(view.inputs[0]).toMatchObject({
      id: 'projected-address-key',
      kind: 'effect',
      address: { kind: 'declared', nodeId: 'reroute-1', portKey: 'input' },
      resolvedType: { display: 'Unknown', resolved: false, dataType: null },
    });
  });
});
