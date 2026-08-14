// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { getCanvasInteraction, useGraphInteractionStore, type ConnectionSession } from '@/features/core/graphInteraction/graphInteractionStore';
import {
  attachCanvasPointerLoop,
  registerCanvasPointerScope,
  type CanvasPointerLoopDeps,
} from './canvasPointerLoop';

const executeCommand = vi.hoisted(() => vi.fn());
const graphWarn = vi.hoisted(() => vi.fn());
vi.mock('@/features/core/history', () => ({ executeCommand }));
vi.mock('@/utils/appLogger', () => ({ logger: { graph: { warn: graphWarn } } }));

const graphPath = 'events/main.yssbi-event';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function connectionSession(sourceId: string): ConnectionSession {
  const source = useGraphDataStore.getState().getGraphPin(graphPath, sourceId)!;
  return {
    groupId: 'group-1',
    graphPath,
    source,
    screenX: 0,
    screenY: 0,
    worldX: 0,
    worldY: 0,
    hoveredTarget: null,
    snappedTarget: null,
    snappedWorld: null,
    feedback: null,
  };
}

describe('canvas pointer loop', () => {
  let detach: (() => void) | undefined;
  let frame: FrameRequestCallback | undefined;
  let activeTabIdRef: { current: string | null };
  let setSelectedNodeIds: ReturnType<typeof vi.fn<CanvasPointerLoopDeps['setSelectedNodeIds']>>;
  let submitConnection: ReturnType<typeof vi.fn<CanvasPointerLoopDeps['submitConnection']>>;
  let reportMutationFailure: ReturnType<typeof vi.fn<CanvasPointerLoopDeps['reportMutationFailure']>>;

  beforeEach(() => {
    vi.clearAllMocks();
    useGraphDataStore.setState({ graphEntities: {} });
    useGraphInteractionStore.setState({ positionOverrides: {}, interactions: {} });
    const fixture = makeEditorProjectionFixture({ graphPath, sourceRevision: 7 });
    fixture.projection.nodes[0].position = { x: 10, y: 20 };
    useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
    const bucket = useGraphDataStore.getState().graphEntities[graphPath];
    const source = Object.values(bucket.pins).find((pin) => pin.direction === 'output')!;
    const target = Object.values(bucket.pins).find((pin) => pin.direction === 'input')!;
    target.nodeId = 'target-node';
    document.body.innerHTML = '<div data-editor-group-id="group-1"></div>';
    const canvas = document.querySelector<HTMLElement>('[data-editor-group-id="group-1"]')!;
    const sourceElement = document.createElement('div');
    sourceElement.dataset.pinId = source.id;
    sourceElement.getBoundingClientRect = () => ({ left: -5, top: -5, width: 10, height: 10, right: 5, bottom: 5, x: -5, y: -5, toJSON: () => ({}) });
    const targetElement = document.createElement('div');
    targetElement.dataset.pinId = target.id;
    targetElement.getBoundingClientRect = () => ({ left: 4, top: 4, width: 10, height: 10, right: 14, bottom: 14, x: 4, y: 4, toJSON: () => ({}) });
    canvas.append(sourceElement, targetElement);
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      frame = callback;
      return 1;
    }));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    activeTabIdRef = { current: graphPath };
    setSelectedNodeIds = vi.fn<CanvasPointerLoopDeps['setSelectedNodeIds']>();
    submitConnection = vi.fn<CanvasPointerLoopDeps['submitConnection']>();
    reportMutationFailure = vi.fn<CanvasPointerLoopDeps['reportMutationFailure']>();
    detach = attachCanvasPointerLoop({
      activeGroupIdRef: { current: 'group-1' },
      activeTabIdRef,
      viewportRef: { current: { x: 0, y: 0, scale: 1 } },
      setSelectedNodeIds,
      persistViewport: vi.fn(),
      setContextMenu: vi.fn(),
      submitConnection,
      reportMutationFailure,
    });
  });

  afterEach(() => {
    detach?.();
    document.body.innerHTML = '';
    vi.unstubAllGlobals();
  });

  it('clears graph selection through the node setter after a blank canvas click', () => {
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'selecting',
      session: {
        groupId: 'group-1',
        startX: 20,
        startY: 30,
        currentX: 20,
        currentY: 30,
        preserveSelection: false,
      },
    });
    registerCanvasPointerScope({ graphPath, groupId: 'group-1' });

    window.dispatchEvent(new PointerEvent('pointerup', { clientX: 20, clientY: 30, button: 0 }));

    expect(setSelectedNodeIds).toHaveBeenCalledOnce();
    expect(setSelectedNodeIds).toHaveBeenCalledWith([], 'group-1');
  });

  it('changes only graph-scoped overrides during pointer movement', () => {
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'draggingNodes',
      session: { groupId: 'group-1', nodeId: 'local-node', lastX: 0, lastY: 0, moved: false, nodeIds: ['local-node'], delta: { x: 0, y: 0 } },
    });
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 5, clientY: 8 }));
    frame?.(0);
    expect(executeCommand).not.toHaveBeenCalled();
    expect(useGraphInteractionStore.getState().positionOverrides[graphPath]?.['local-node']).toEqual({ x: 15, y: 28 });
    expect(useGraphDataStore.getState().graphEntities[graphPath].nodes['local-node'].position).toEqual({ x: 10, y: 20 });
  });

  it.each([
    ['drawingConnection', 'ConnectPins'],
    ['movingConnections', 'MoveConnections'],
  ] as const)('submits one %s release intent from snapped pins', async (type, command) => {
    const pins = Object.values(useGraphDataStore.getState().graphEntities[graphPath].pins);
    const source = pins.find((pin) => pin.direction === 'output')!;
    const target = pins.find((pin) => pin.direction === 'input')!;
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type,
      session: {
        ...connectionSession(source.id),
        snappedTarget: target,
        feedback: { kind: 'append' },
      },
    });
    submitConnection.mockResolvedValue({ status: 'applied' });

    window.dispatchEvent(new PointerEvent('pointerup', { clientX: 9, clientY: 9, button: 0 }));
    await Promise.resolve();

    expect(submitConnection).toHaveBeenCalledOnce();
    expect(submitConnection).toHaveBeenCalledWith({
      graphPath,
      intent: command === 'ConnectPins' ? 'connect' : 'moveConnections',
      sourcePinId: source.id,
      targetPinId: target.id,
    });
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1')).toEqual({ type: 'idle' });
  });

  it('resolves replacement from production canvas geometry during rAF with zero pointermove IPC', async () => {
    const bucket = useGraphDataStore.getState().graphEntities[graphPath];
    const source = Object.values(bucket.pins).find((pin) => pin.direction === 'output')!;
    const target = Object.values(bucket.pins).find((pin) => pin.direction === 'input')!;
    target.nodeId = 'target-node';
    target.address = { kind: 'declared', nodeId: 'target-node', portKey: 'target-in' };
    bucket.pinConnections[source.id] = ['source-z', 'shared'];
    bucket.pinConnections[target.id] = ['target-a', 'shared'];

    const canvas = document.querySelector<HTMLElement>('[data-editor-group-id="group-1"]')!;
    const sourceElement = document.createElement('div');
    sourceElement.dataset.pinId = source.id;
    sourceElement.getBoundingClientRect = () => ({ left: 0, top: 0, width: 10, height: 10, right: 10, bottom: 10, x: 0, y: 0, toJSON: () => ({}) });
    const targetElement = document.createElement('div');
    targetElement.dataset.pinId = target.id;
    targetElement.getBoundingClientRect = () => ({ left: 95, top: 95, width: 10, height: 10, right: 105, bottom: 105, x: 95, y: 95, toJSON: () => ({}) });
    canvas.append(sourceElement, targetElement);

    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'drawingConnection',
      session: connectionSession(source.id),
    });
    registerCanvasPointerScope({ graphPath, groupId: 'group-1' });

    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 100, clientY: 100 }));
    expect(submitConnection).not.toHaveBeenCalled();
    frame?.(0);

    const resolved = getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1');
    expect(resolved.type).toBe('drawingConnection');
    if (resolved.type !== 'drawingConnection') throw new Error('expected drawing interaction');
    expect(resolved.session.snappedTarget?.id).toBe(target.id);
    expect(resolved.session.feedback).toEqual({
      kind: 'replace',
      displacedConnectionIds: ['shared', 'source-z', 'target-a'],
    });
    expect(submitConnection).not.toHaveBeenCalled();

    submitConnection.mockResolvedValue({ status: 'applied' });
    window.dispatchEvent(new PointerEvent('pointerup', { clientX: 100, clientY: 100, button: 0 }));
    await Promise.resolve();
    expect(submitConnection).toHaveBeenCalledOnce();
    expect(submitConnection).toHaveBeenCalledWith({
      graphPath,
      intent: 'connect',
      sourcePinId: source.id,
      targetPinId: target.id,
    });
  });

  it('submits replacement as one ConnectPins intent while IDs remain visual only', async () => {
    const pins = Object.values(useGraphDataStore.getState().graphEntities[graphPath].pins);
    const source = pins.find((pin) => pin.direction === 'output')!;
    const target = pins.find((pin) => pin.direction === 'input')!;
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'drawingConnection',
      session: {
        ...connectionSession(source.id),
        snappedTarget: target,
        feedback: { kind: 'replace', displacedConnectionIds: ['visual-only-id'] },
      },
    });
    submitConnection.mockResolvedValue({ status: 'applied' });
    window.dispatchEvent(new PointerEvent('pointerup', { button: 0 }));
    await Promise.resolve();
    expect(submitConnection).toHaveBeenCalledWith({ graphPath, intent: 'connect', sourcePinId: source.id, targetPinId: target.id });
  });

  it('keeps pointer-down graph scope when the active tab changes before release', async () => {
    const pins = Object.values(useGraphDataStore.getState().graphEntities[graphPath].pins);
    const source = pins.find((pin) => pin.direction === 'output')!;
    const target = pins.find((pin) => pin.direction === 'input')!;
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'drawingConnection',
      session: { ...connectionSession(source.id), snappedTarget: target, feedback: { kind: 'append' } },
    });
    registerCanvasPointerScope({ graphPath, groupId: 'group-1' });
    submitConnection.mockResolvedValue({ status: 'applied' });
    activeTabIdRef.current = 'events/other.yssbi-event';

    window.dispatchEvent(new PointerEvent('pointerup', { button: 0 }));
    await Promise.resolve();

    expect(submitConnection).toHaveBeenCalledWith({ graphPath, intent: 'connect', sourcePinId: source.id, targetPinId: target.id });
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1')).toEqual({ type: 'idle' });
  });

  it('cancels safely and clears pointer scope when the graph bucket disappears before a frame', () => {
    const source = Object.values(useGraphDataStore.getState().graphEntities[graphPath].pins)[0];
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'drawingConnection',
      session: connectionSession(source.id),
    });
    registerCanvasPointerScope({ graphPath, groupId: 'group-1' });
    useGraphDataStore.getState().clearGraph(graphPath);

    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 5, clientY: 5 }));
    expect(() => frame?.(0)).not.toThrow();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1')).toEqual({ type: 'idle' });
  });

  it.each(['drawingConnection', 'movingConnections'] as const)(
    'Escape-style cancellation of %s emits zero commands',
    (type) => {
      const source = Object.values(useGraphDataStore.getState().graphEntities[graphPath].pins)[0];
      useGraphInteractionStore.getState().startInteraction(graphPath, {
        type,
        session: connectionSession(source.id),
      });
      useGraphInteractionStore.getState().cancelInteraction(graphPath, 'group-1');
      window.dispatchEvent(new PointerEvent('pointerup', { button: 0 }));
      expect(submitConnection).not.toHaveBeenCalled();
    },
  );

  it('opens pending node creation when source and invalid candidates are outside the hover radius', () => {
    const bucket = useGraphDataStore.getState().graphEntities[graphPath];
    const source = Object.values(bucket.pins).find((pin) => pin.direction === 'output')!;
    const setContextMenu = vi.fn();
    detach?.();
    detach = attachCanvasPointerLoop({
      activeGroupIdRef: { current: 'group-1' },
      activeTabIdRef,
      viewportRef: { current: { x: 0, y: 0, scale: 1 } },
      setSelectedNodeIds: vi.fn(),
      persistViewport: vi.fn(),
      setContextMenu,
      submitConnection,
      reportMutationFailure,
    });
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'drawingConnection',
      session: connectionSession(source.id),
    });
    registerCanvasPointerScope({ graphPath, groupId: 'group-1' });

    window.dispatchEvent(new PointerEvent('pointerup', { clientX: 500, clientY: 500, button: 0 }));

    expect(submitConnection).not.toHaveBeenCalled();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1').type)
      .toBe('pendingNodeCreation');
    expect(setContextMenu).toHaveBeenCalledWith({ x: 500, y: 500, visible: true });
  });

  it('invalid release emits no command and returns idle', () => {
    const pins = Object.values(useGraphDataStore.getState().graphEntities[graphPath].pins);
    const source = pins.find((pin) => pin.direction === 'output')!;
    const target = pins.find((pin) => pin.direction === 'input')!;
    target.nodeId = source.nodeId;
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'drawingConnection',
      session: connectionSession(source.id),
    });
    window.dispatchEvent(new PointerEvent('pointerup', { clientX: 9, clientY: 9, button: 0 }));
    expect(submitConnection).not.toHaveBeenCalled();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1')).toEqual({ type: 'idle' });
  });

  it('reports only the application-provided safe failure outcome', async () => {
    const pins = Object.values(useGraphDataStore.getState().graphEntities[graphPath].pins);
    const source = pins.find((pin) => pin.direction === 'output')!;
    const target = pins.find((pin) => pin.direction === 'input')!;
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'drawingConnection',
      session: { ...connectionSession(source.id), snappedTarget: target, feedback: { kind: 'append' } },
    });
    submitConnection.mockResolvedValue({
      status: 'failed',
      message: 'Connection types are incompatible',
    });

    window.dispatchEvent(new PointerEvent('pointerup', { button: 0 }));
    await Promise.resolve();
    await Promise.resolve();

    expect(reportMutationFailure).toHaveBeenCalledOnce();
    expect(reportMutationFailure).toHaveBeenCalledWith({
      graphPath,
      intent: 'connect',
      message: 'Connection types are incompatible',
    });
    expect(JSON.stringify(reportMutationFailure.mock.calls)).not.toContain('graph_connection_type_mismatch');
  });

  it.each(['success', 'failure'] as const)(
    'submits one final MoveNodes intent and clears overrides after %s settles',
    async (result) => {
      const pending = deferred<boolean>();
      executeCommand.mockReturnValueOnce(pending.promise);
      useGraphInteractionStore.getState().startInteraction(graphPath, {
        type: 'draggingNodes',
        session: { groupId: 'group-1', nodeId: 'local-node', lastX: 0, lastY: 0, moved: false, nodeIds: ['local-node'], delta: { x: 0, y: 0 } },
      });
      registerCanvasPointerScope({ graphPath, groupId: 'group-1' });

      window.dispatchEvent(new PointerEvent('pointermove', { clientX: 5, clientY: 8 }));
      frame?.(0);
      window.dispatchEvent(new PointerEvent('pointerup', { clientX: 5, clientY: 8, button: 0 }));

      expect(executeCommand).toHaveBeenCalledOnce();
      expect(executeCommand).toHaveBeenCalledWith(graphPath, 'MoveNodes', {
        positions: [{ nodeId: 'local-node', position: { x: 15, y: 28 } }],
      });
      expect(useGraphInteractionStore.getState().positionOverrides[graphPath]?.['local-node']).toEqual({ x: 15, y: 28 });

      if (result === 'success') pending.resolve(true);
      else pending.reject(new Error('raw UUID 00000000-0000-0000-0000-000000000000'));
      await pending.promise.catch(() => undefined);
      await Promise.resolve();

      expect(useGraphInteractionStore.getState().positionOverrides[graphPath]).toBeUndefined();
      if (result === 'failure') {
        expect(graphWarn).toHaveBeenCalledWith(
          `MoveNodes command failed graphPath=${graphPath}`,
          'CanvasInteraction',
        );
        expect(JSON.stringify(graphWarn.mock.calls)).not.toContain('00000000');
      }
    },
  );
});
