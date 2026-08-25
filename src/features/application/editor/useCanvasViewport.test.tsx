// @vitest-environment happy-dom

import { act, createElement, useRef } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useCanvasViewport } from './useCanvasViewport';

const mocks = vi.hoisted(() => ({
  resolvePinOffsetWaiters: vi.fn(),
  measurePinConnectionAnchor: vi.fn(() => ({
    pinId: 'pin-1',
    center: { x: 12, y: 18 },
  })),
}));

vi.mock('@/features/core/viewport', () => ({
  editorViewportScope: (groupId: string, graphPath: string) => ({ groupId, graphPath }),
  getViewport: () => ({ x: 0, y: 0, scale: 1 }),
}));

vi.mock('@/features/core/dataStore', () => {
  const graphNodeIds = ['node-1'];
  const state = {
    getGraphNodeIds: () => graphNodeIds,
    getGraphNode: () => ({ position: { x: 0, y: 0 } }),
    getGraphNodePins: () => ['pin-1'],
  };
  return {
    useGraphDataStore: (selector: (value: typeof state) => unknown) => selector(state),
  };
});

vi.mock('@/features/core/graphInteraction', () => ({
  useGraphInteractionStore: {
    getState: () => ({ positionOverrides: {} }),
  },
}));

vi.mock('@/features/core/canvas/pinConnectionAnchor', () => ({
  measurePinConnectionAnchor: mocks.measurePinConnectionAnchor,
}));

vi.mock('@/features/core/canvas/pinOffsetWaiter', () => ({
  resolvePinOffsetWaiters: mocks.resolvePinOffsetWaiters,
}));

type ObserverRecord = {
  readonly observed: Element[];
};

class FakeResizeObserver {
  static readonly records: ObserverRecord[] = [];
  readonly observed: Element[] = [];

  constructor(_: ResizeObserverCallback) {
    FakeResizeObserver.records.push(this);
  }

  observe(element: Element): void {
    this.observed.push(element);
  }

  disconnect(): void {}
}

describe('useCanvasViewport', () => {
  let host: HTMLDivElement;
  let root: Root;

  function Harness() {
    const canvasRef = useRef<HTMLDivElement>(null);
    useCanvasViewport(canvasRef, 'group-1', 'events/Main.yssbi-event');
    return createElement(
      'div',
      { ref: canvasRef },
      createElement(
        'div',
        { 'data-node-id': 'node-1' },
        createElement('div', { 'data-pin-id': 'pin-1' }),
      ),
    );
  }

  beforeEach(() => {
    vi.clearAllMocks();
    FakeResizeObserver.records.length = 0;
    vi.stubGlobal('ResizeObserver', FakeResizeObserver);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  });

  it('observes the canvas root so reattached panels can be measured again', async () => {
    await act(async () => {
      root.render(createElement(Harness));
      await Promise.resolve();
    });

    const canvas = host.firstElementChild;
    expect(canvas).not.toBeNull();
    expect(FakeResizeObserver.records[0]?.observed).toContain(canvas);
  });
});
