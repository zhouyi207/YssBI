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
    pointerId: 0,
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

function appendSelectionNode(
  id: string,
  bounds: { left: number; top: number; right: number; bottom: number },
): HTMLElement {
  const canvas = document.querySelector<HTMLElement>('[data-editor-group-id="group-1"]')!;
  const node = document.createElement('div');
  node.dataset.nodeId = id;
  node.getBoundingClientRect = () => ({
    ...bounds,
    width: bounds.right - bounds.left,
    height: bounds.bottom - bounds.top,
    x: bounds.left,
    y: bounds.top,
    toJSON: () => ({}),
  });
  canvas.append(node);
  return node;
}

function appendPinElement(
  canvas: HTMLElement,
  pinId: string,
  bounds: { left: number; top: number; width: number; height: number },
): void {
  const pin = document.createElement('div');
  pin.dataset.pinId = pinId;
  const anchor = document.createElement('div');
  anchor.dataset.pinConnectionAnchor = pinId;
  anchor.getBoundingClientRect = () => ({
    ...bounds,
    right: bounds.left + bounds.width,
    bottom: bounds.top + bounds.height,
    x: bounds.left,
    y: bounds.top,
    toJSON: () => ({}),
  });
  pin.append(anchor);
  canvas.append(pin);
}

function startSelection(
  baseNodeIds: readonly string[],
  startX = 0,
  startY = 0,
  pointerId = 0,
): void {
  useGraphInteractionStore.getState().startInteraction(graphPath, {
    type: 'selecting',
    session: {
      groupId: 'group-1',
      pointerId,
      startX,
      startY,
      currentX: startX,
      currentY: startY,
      baseNodeIds,
    },
  });
  registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId });
}

describe('canvas pointer loop', () => {
  let detach: (() => void) | undefined;
  let frame: FrameRequestCallback | undefined;
  let activeTabIdRef: { current: string | null };
  let setSelectedNodeIds: ReturnType<typeof vi.fn<CanvasPointerLoopDeps['setSelectedNodeIds']>>;
  let setContextMenu: ReturnType<typeof vi.fn<CanvasPointerLoopDeps['setContextMenu']>>;
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
    appendPinElement(canvas, source.id, { left: -5, top: -5, width: 10, height: 10 });
    appendPinElement(canvas, target.id, { left: 4, top: 4, width: 10, height: 10 });
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      frame = callback;
      return 1;
    }));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    activeTabIdRef = { current: graphPath };
    setSelectedNodeIds = vi.fn<CanvasPointerLoopDeps['setSelectedNodeIds']>();
    setContextMenu = vi.fn<CanvasPointerLoopDeps['setContextMenu']>();
    submitConnection = vi.fn<CanvasPointerLoopDeps['submitConnection']>();
    reportMutationFailure = vi.fn<CanvasPointerLoopDeps['reportMutationFailure']>();
    detach = attachCanvasPointerLoop({
      activeGroupIdRef: { current: 'group-1' },
      activeTabIdRef,
      viewportRef: { current: { x: 0, y: 0, scale: 1 } },
      setSelectedNodeIds,
      persistViewport: vi.fn(),
      setContextMenu,
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
        pointerId: 0,
        startX: 20,
        startY: 30,
        currentX: 20,
        currentY: 30,
        baseNodeIds: [],
      },
    });
    registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });

    window.dispatchEvent(new PointerEvent('pointerup', { clientX: 20, clientY: 30, button: 0 }));

    expect(setSelectedNodeIds).toHaveBeenCalledOnce();
    expect(setSelectedNodeIds).toHaveBeenCalledWith([], 'group-1');
  });

  it('uses the Shift selection captured at pointerdown for preview and finalization', () => {
    const nodeA = appendSelectionNode('a', { left: 200, top: 200, right: 220, bottom: 220 });
    const nodeB = appendSelectionNode('b', { left: 10, top: 10, right: 20, bottom: 20 });
    const nodeC = appendSelectionNode('c', { left: 30, top: 30, right: 40, bottom: 40 });
    startSelection(['a', 'b']);

    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 50, clientY: 50, button: 0 }));
    frame?.(0);

    expect(nodeA.dataset.selectionPreview).toBe('true');
    expect(nodeB.dataset.selectionPreview).toBe('true');
    expect(nodeC.dataset.selectionPreview).toBe('true');

    setSelectedNodeIds(['store-change'], 'group-1');
    setSelectedNodeIds.mockClear();
    window.dispatchEvent(new PointerEvent('pointerup', { clientX: 50, clientY: 50, button: 0 }));

    expect(setSelectedNodeIds).toHaveBeenCalledOnce();
    expect(setSelectedNodeIds).toHaveBeenCalledWith(['a', 'b', 'c'], 'group-1');
  });

  it('preserves the session-start selection after an empty Shift drag', () => {
    startSelection(['a', 'b']);

    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 50, clientY: 50, button: 0 }));
    frame?.(0);
    window.dispatchEvent(new PointerEvent('pointerup', { clientX: 50, clientY: 50, button: 0 }));

    expect(setSelectedNodeIds).toHaveBeenCalledWith(['a', 'b'], 'group-1');
  });

  it('replaces selection with current hits after a plain drag', () => {
    appendSelectionNode('hit-a', { left: 10, top: 10, right: 20, bottom: 20 });
    appendSelectionNode('hit-b', { left: 30, top: 30, right: 40, bottom: 40 });
    startSelection([]);

    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 50, clientY: 50, button: 0 }));
    frame?.(0);
    window.dispatchEvent(new PointerEvent('pointerup', { clientX: 50, clientY: 50, button: 0 }));

    expect(setSelectedNodeIds).toHaveBeenCalledWith(['hit-a', 'hit-b'], 'group-1');
  });

  it('preserves the session-start selection after a Shift blank click', () => {
    startSelection(['a', 'b'], 20, 30);

    window.dispatchEvent(new PointerEvent('pointerup', { clientX: 20, clientY: 30, button: 0 }));

    expect(setSelectedNodeIds).toHaveBeenCalledWith(['a', 'b'], 'group-1');
  });

  it('cancels selection without preview or final writes when the initiating group switches graphs', () => {
    const node = appendSelectionNode('hit', { left: 10, top: 10, right: 20, bottom: 20 });
    startSelection(['base'], 0, 0, 7);
    activeTabIdRef.current = 'events/other.yssbi-event';

    window.dispatchEvent(new PointerEvent('pointermove', { pointerId: 7, clientX: 50, clientY: 50 }));
    frame?.(0);
    window.dispatchEvent(new PointerEvent('pointerup', { pointerId: 7, clientX: 50, clientY: 50 }));

    expect(node.dataset.selectionPreview).toBeUndefined();
    expect(setSelectedNodeIds).not.toHaveBeenCalled();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1')).toEqual({ type: 'idle' });
  });

  it('does not restore a Shift blank-click baseline after the initiating group switches graphs', () => {
    startSelection(['base'], 20, 30, 7);
    activeTabIdRef.current = 'events/other.yssbi-event';

    window.dispatchEvent(new PointerEvent('pointerup', { pointerId: 7, clientX: 20, clientY: 30 }));

    expect(setSelectedNodeIds).not.toHaveBeenCalled();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1')).toEqual({ type: 'idle' });
  });

  it('ignores foreign pointer movement and release', () => {
    const node = appendSelectionNode('hit', { left: 10, top: 10, right: 20, bottom: 20 });
    startSelection([], 0, 0, 7);

    window.dispatchEvent(new PointerEvent('pointermove', { pointerId: 8, clientX: 50, clientY: 50 }));
    frame?.(0);
    window.dispatchEvent(new PointerEvent('pointerup', { pointerId: 8, clientX: 50, clientY: 50 }));
    window.dispatchEvent(new PointerEvent('pointercancel', { pointerId: 8 }));

    expect(node.dataset.selectionPreview).toBeUndefined();
    expect(setSelectedNodeIds).not.toHaveBeenCalled();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1').type).toBe('selecting');

    window.dispatchEvent(new PointerEvent('pointerup', { pointerId: 7, clientX: 0, clientY: 0 }));
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1')).toEqual({ type: 'idle' });
  });

  it('cancels the owning pointer on pointercancel and clears preview without writes', () => {
    const node = appendSelectionNode('hit', { left: 10, top: 10, right: 20, bottom: 20 });
    startSelection([], 0, 0, 7);
    window.dispatchEvent(new PointerEvent('pointermove', { pointerId: 7, clientX: 50, clientY: 50 }));
    frame?.(0);
    expect(node.dataset.selectionPreview).toBe('true');

    window.dispatchEvent(new PointerEvent('pointercancel', { pointerId: 7 }));

    expect(node.dataset.selectionPreview).toBeUndefined();
    expect(setSelectedNodeIds).not.toHaveBeenCalled();
    expect(submitConnection).not.toHaveBeenCalled();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1')).toEqual({ type: 'idle' });
  });

  it('cancels its scoped interaction and preview when the final pointer loop detaches', () => {
    const node = appendSelectionNode('hit', { left: 10, top: 10, right: 20, bottom: 20 });
    startSelection([], 0, 0, 7);
    window.dispatchEvent(new PointerEvent('pointermove', { pointerId: 7, clientX: 50, clientY: 50 }));
    frame?.(0);
    expect(node.dataset.selectionPreview).toBe('true');

    detach?.();
    detach = undefined;

    expect(node.dataset.selectionPreview).toBeUndefined();
    expect(setSelectedNodeIds).not.toHaveBeenCalled();
    expect(submitConnection).not.toHaveBeenCalled();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1')).toEqual({ type: 'idle' });
  });

  it('opens the canvas context menu when right-click movement stays below the drag threshold', () => {
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'panning',
      session: { groupId: 'group-1', pointerId: 0, startX: 20, startY: 30, lastX: 20, lastY: 30, moved: false },
    });
    registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });

    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 22, clientY: 31, button: 2 }));
    frame?.(0);
    window.dispatchEvent(new PointerEvent('pointerup', { clientX: 22, clientY: 31, button: 2 }));

    expect(setContextMenu).toHaveBeenCalledWith({ x: 22, y: 31, visible: true });
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1').type).toBe('idle');
  });

  it('changes only graph-scoped overrides during pointer movement', () => {
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'draggingNodes',
      session: { groupId: 'group-1', pointerId: 0, nodeId: 'local-node', lastX: 0, lastY: 0, moved: false, nodeIds: ['local-node'], delta: { x: 0, y: 0 } },
    });
    registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 5, clientY: 8 }));
    frame?.(0);
    expect(executeCommand).not.toHaveBeenCalled();
    expect(useGraphInteractionStore.getState().positionOverrides[graphPath]?.['local-node']).toEqual({ x: 15, y: 28 });
    expect(useGraphDataStore.getState().graphEntities[graphPath].nodes['local-node'].position).toEqual({ x: 10, y: 20 });
  });

  it('does not submit a node move when the pointer is released without moving', () => {
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'draggingNodes',
      session: { groupId: 'group-1', pointerId: 0, nodeId: 'local-node', lastX: 10, lastY: 20, moved: false, nodeIds: ['local-node'], delta: { x: 0, y: 0 } },
    });
    registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });

    window.dispatchEvent(new PointerEvent('pointerup', { clientX: 10, clientY: 20, button: 0 }));

    expect(executeCommand).not.toHaveBeenCalled();
    expect(useGraphInteractionStore.getState().positionOverrides[graphPath]).toBeUndefined();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-1')).toEqual({ type: 'idle' });
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
    registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });
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
    appendPinElement(canvas, source.id, { left: 0, top: 0, width: 10, height: 10 });
    appendPinElement(canvas, target.id, { left: 95, top: 95, width: 10, height: 10 });

    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'drawingConnection',
      session: connectionSession(source.id),
    });
    registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });

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
    registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });
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
    registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });
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
    registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });
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
    registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });

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
    registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });
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
    registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });
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
        session: { groupId: 'group-1', pointerId: 0, nodeId: 'local-node', lastX: 0, lastY: 0, moved: false, nodeIds: ['local-node'], delta: { x: 0, y: 0 } },
      });
      registerCanvasPointerScope({ graphPath, groupId: 'group-1', pointerId: 0 });

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
