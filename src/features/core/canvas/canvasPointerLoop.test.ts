// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { useGestureStore } from '@/features/core/gesture';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useGraphInteractionStore } from '@/features/core/graphInteraction/graphInteractionStore';
import { attachCanvasPointerLoop } from './canvasPointerLoop';

const executeCommand = vi.hoisted(() => vi.fn());

vi.mock('@/features/core/history', () => ({ executeCommand }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('canvas drag mutation loop', () => {
  let detach: (() => void) | undefined;
  let frame: FrameRequestCallback | undefined;

  beforeEach(() => {
    vi.clearAllMocks();
    useGraphDataStore.setState({ graphEntities: {} });
    useGraphInteractionStore.setState({ positionOverrides: {} });
    useGestureStore.setState({ gesture: null, suppressNextContextMenu: false });
    const fixture = makeEditorProjectionFixture({
      graphPath: 'events/main.yssbi-event',
      sourceRevision: 7,
    });
    fixture.projection.nodes[0].position = { x: 10, y: 20 };
    useGraphDataStore.getState().replaceProjection(
      'events/main.yssbi-event',
      fixture.projection,
      1,
    );
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      frame = callback;
      return 1;
    }));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    detach = attachCanvasPointerLoop({
      activeGroupIdRef: { current: 'group-1' },
      activeTabIdRef: { current: 'events/main.yssbi-event' },
      viewportRef: { current: { x: 0, y: 0, scale: 1 } },
      setSelectedNodeIds: vi.fn(),
      connectPins: vi.fn(async () => undefined),
      persistViewport: vi.fn(),
      setContextMenu: vi.fn(),
      setPendingConnection: vi.fn(),
    });
    useGestureStore.getState().setGesture({
      type: 'drag',
      nodeId: 'local-node',
      lastX: 0,
      lastY: 0,
      moved: false,
      groupId: 'group-1',
      dragNodeIds: ['local-node'],
      dragDelta: { x: 0, y: 0 },
    });
  });

  afterEach(() => {
    detach?.();
    vi.unstubAllGlobals();
  });

  it('changes only graph-scoped overrides during pointer movement', () => {
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 5, clientY: 8 }));
    frame?.(0);

    expect(useGraphInteractionStore.getState().positionOverrides).toEqual({
      'events/main.yssbi-event': { 'local-node': { x: 15, y: 28 } },
    });
    const bucket = useGraphDataStore.getState().graphEntities['events/main.yssbi-event'];
    expect(bucket.nodes['local-node'].position).toEqual({ x: 10, y: 20 });
    expect(bucket.sourceRevision).toBe(7);
  });

  it.each(['success', 'failure'] as const)(
    'submits one final MoveNodes intent and clears overrides on %s',
    async (result) => {
      const pending = deferred<unknown>();
      executeCommand.mockReturnValueOnce(pending.promise);
      window.dispatchEvent(new PointerEvent('pointermove', { clientX: 5, clientY: 8 }));
      frame?.(0);

      window.dispatchEvent(new PointerEvent('pointerup', { clientX: 5, clientY: 8, button: 0 }));

      expect(executeCommand).toHaveBeenCalledTimes(1);
      expect(executeCommand).toHaveBeenCalledWith(
        'events/main.yssbi-event',
        'MoveNodes',
        { positions: [{ nodeId: 'local-node', position: { x: 15, y: 28 } }] },
      );
      expect(
        useGraphInteractionStore.getState().positionOverrides['events/main.yssbi-event']?.['local-node'],
      ).toEqual({ x: 15, y: 28 });

      if (result === 'success') pending.resolve({ status: 'applied' });
      else pending.reject(new Error('mutation failed'));
      await pending.promise.catch(() => undefined);
      await Promise.resolve();

      expect(useGraphInteractionStore.getState().positionOverrides).toEqual({});
    },
  );
});
