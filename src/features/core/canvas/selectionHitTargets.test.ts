// @vitest-environment happy-dom

import { describe, expect, it } from 'vitest';
import { unionSelectionIds } from './selectionSession';
import { collectSelectionHitTargets, hitTestSelection } from './selectionHitTargets';
import type { SelectionHitTarget } from './selectionHitTargets';


describe('unionSelectionIds', () => {
  it('keeps base order and appends unseen hits in hit-test order', () => {
    expect(unionSelectionIds(['a', 'b'], ['b', 'c'])).toEqual(['a', 'b', 'c']);
  });

  it('deduplicates repeated hit IDs', () => {
    expect(unionSelectionIds(['a'], ['b', 'b', 'a', 'c', 'c'])).toEqual(['a', 'b', 'c']);
  });
});

describe('hitTestSelection', () => {
  const targets: SelectionHitTarget[] = [
    { id: 'a', left: 10, right: 50, top: 10, bottom: 50 },
    { id: 'b', left: 100, right: 140, top: 10, bottom: 50 },
  ];

  it('returns ids intersecting the rect', () => {
    expect(hitTestSelection(targets, { x1: 0, y1: 0, x2: 60, y2: 60 })).toEqual(['a']);
    expect(hitTestSelection(targets, { x1: 90, y1: 0, x2: 150, y2: 60 })).toEqual(['b']);
    expect(hitTestSelection(targets, { x1: 0, y1: 0, x2: 200, y2: 60 })).toEqual(['a', 'b']);
  });

  it('collects node hit targets without treating interactive edge paths as box-selectable', () => {
    const canvas = document.createElement('div');
    const node = document.createElement('div');
    node.dataset.nodeId = 'node-a';
    node.getBoundingClientRect = () => ({
      left: 10,
      right: 50,
      top: 20,
      bottom: 60,
      width: 40,
      height: 40,
      x: 10,
      y: 20,
      toJSON: () => ({}),
    });
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    const edge = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    edge.setAttribute('data-edge-hit-target', 'edge-a');
    svg.appendChild(edge);
    canvas.append(node, svg);

    expect(collectSelectionHitTargets(canvas)).toEqual([{
      id: 'node-a',
      left: 10,
      right: 50,
      top: 20,
      bottom: 60,
    }]);
  });

  it('returns empty when nothing intersects', () => {
    expect(hitTestSelection(targets, { x1: 200, y1: 200, x2: 300, y2: 300 })).toEqual([]);
  });
});
