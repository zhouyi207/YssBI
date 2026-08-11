// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { uiStore } from '@/features/core/ui/UIStore';
import { logger } from '@/utils/appLogger';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { useEditorStore } from '@/features/core/editor';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import type { Pin } from '@/shared/types/domain/pin';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import { createNodeFromDescriptor } from '@/features/application/nodeCatalog/createNodeFromDescriptor';
import { useCanvasDrop } from './useCanvasDrop';
import { useCanvasOverlayHandlers } from './useCanvasOverlayHandlers';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const executeCommand = vi.hoisted(() => vi.fn());
vi.mock('@/features/core/history', () => ({ executeCommand }));
vi.mock('@/features/application/nodeCatalog/createNodeFromDescriptor', () => ({
  createNodeFromDescriptor: vi.fn().mockResolvedValue({ status: 'conflict' }),
}));

const resourceDescriptor: NodeCreationDescriptor = {
  kind: 'resourceBound',
  nodeTypeId: 'functions.call',
  resourcePath: 'functions/helper.yssbi-function',
  resourceRevision: 3,
  createArgs: { kind: 'function' },
};
const refreshCatalog = vi.fn();
vi.mock('@/features/application/nodeCatalog/useLocalizedNodeCatalog', () => ({
  useLocalizedNodeCatalog: () => ({
    status: 'ready',
    error: null,
    catalog: {
      projectInstanceId: 'project-1',
      registryFingerprint: 'registry-1',
      resourcePublicationRevision: 3,
      locale: 'en-US',
      categories: [],
      items: [{
        nodeTypeId: 'functions.call', title: 'Helper', description: null, documentation: null,
        categoryId: 'functions', iconId: 'function', styleId: 'call', aliases: [],
        technicalTerms: [], backendSearchText: ['helper'], resourceNames: ['Helper'],
        ports: [], parameters: [],
        resourcePath: 'functions/helper.yssbi-function', resourceRevision: 3,
        creation: resourceDescriptor,
      }],
    },
    searchIndex: null,
    refresh: refreshCatalog,
  }),
}));

const graphPath = 'events/main.yssbi-event';
const groupId = 'group-1';
const sourceAddress = {
  kind: 'declared' as const,
  nodeId: '00000000-0000-0000-0000-000000000101',
  portKey: 'output',
};
const newerSourceAddress = {
  kind: 'declared' as const,
  nodeId: '00000000-0000-0000-0000-000000000102',
  portKey: 'new-output',
};

function pin(address = sourceAddress): Pin {
  return {
    id: 'output',
    nodeId: 'source',
    name: 'Output',
    direction: 'output',
    type: 'object',
    address,
  } as Pin;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

describe('unavailable creation routing', () => {
  let host: HTMLDivElement;
  let canvas: HTMLDivElement;
  let root: Root;
  const createNode = vi.fn();
  const showToast = vi.spyOn(uiStore, 'showToast');
  const logError = vi.spyOn(logger.graph, 'error').mockImplementation(() => {});

  beforeEach(() => {
    vi.clearAllMocks();
    useEditorStore.setState({ contextMenu: null, pendingConnection: null });
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useEditorTabStore.getState().initGroupPlacement(groupId, [{
      id: graphPath,
      component: 'GraphEditor',
      type: 'event',
    }], graphPath);
    host = document.createElement('div');
    canvas = document.createElement('div');
    canvas.getBoundingClientRect = () => ({
      x: 0,
      y: 0,
      left: 0,
      top: 0,
      right: 500,
      bottom: 500,
      width: 500,
      height: 500,
      toJSON: () => ({}),
    });
    host.appendChild(canvas);
    document.body.appendChild(host);
    root = createRoot(document.createElement('div'));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it.each([
    { name: 'palette selection', pendingConnection: null },
    { name: 'edge-drop palette selection', pendingConnection: pin() },
  ])('routes static $name through descriptor creation', async ({ pendingConnection }) => {
    let select!: ReturnType<typeof useCanvasOverlayHandlers>['handleNodePaletteSelect'];
    const setContextMenu = vi.fn();
    const setPendingConnection = vi.fn();
    function Harness() {
      select = useCanvasOverlayHandlers({
        canvasElementRef: { current: canvas },
        groupId,
        activeTabId: graphPath,
        pendingConnection,
        setContextMenu,
        setPendingConnection,
      }).handleNodePaletteSelect;
      return null;
    }
    act(() => root.render(<Harness />));

    await act(async () => {
      await select(
        { kind: 'static', nodeTypeId: 'math.add' },
        'zh-CN',
        { x: 20, y: 30 },
      );
    });

    expect(createNodeFromDescriptor).toHaveBeenCalledWith({
      graphPath,
      locale: 'zh-CN',
      descriptor: { kind: 'static', nodeTypeId: 'math.add' },
      position: { x: 20, y: 30 },
      connectFrom: pendingConnection ? sourceAddress : null,
    });
    expect(createNode).not.toHaveBeenCalled();
    expect(executeCommand).not.toHaveBeenCalled();
    expect(showToast).not.toHaveBeenCalled();
    expect(setContextMenu).not.toHaveBeenCalled();
    expect(setPendingConnection).not.toHaveBeenCalled();
  });

  it('keeps pending state until atomic create/connect is applied, then clears it', async () => {
    const pending = deferred<{ status: 'applied'; result: never }>();
    vi.mocked(createNodeFromDescriptor).mockReturnValueOnce(pending.promise);
    const setContextMenu = vi.fn();
    const setPendingConnection = vi.fn();
    let select!: ReturnType<typeof useCanvasOverlayHandlers>['handleNodePaletteSelect'];
    function Harness() {
      select = useCanvasOverlayHandlers({
        canvasElementRef: { current: canvas },
        groupId,
        activeTabId: graphPath,
        pendingConnection: pin(),
        setContextMenu,
        setPendingConnection,
      }).handleNodePaletteSelect;
      return null;
    }
    act(() => root.render(<Harness />));
    useEditorStore.setState({
      contextMenu: { x: 20, y: 30, visible: true },
      pendingConnection: pin(),
    });

    const selection = select(
      { kind: 'static', nodeTypeId: 'math.add' },
      'en-US',
      { x: 20, y: 30 },
    );
    expect(setContextMenu).not.toHaveBeenCalled();
    expect(setPendingConnection).not.toHaveBeenCalled();

    pending.resolve({ status: 'applied', result: {} as never });
    await act(async () => selection);

    expect(setContextMenu).toHaveBeenCalledWith(null);
    expect(setPendingConnection).toHaveBeenCalledWith(null);
  });

  it('does not let an older applied create clear a newer palette interaction', async () => {
    const pending = deferred<{ status: 'applied'; result: never }>();
    vi.mocked(createNodeFromDescriptor).mockReturnValueOnce(pending.promise);
    const setContextMenu = vi.fn();
    const setPendingConnection = vi.fn();
    let select!: ReturnType<typeof useCanvasOverlayHandlers>['handleNodePaletteSelect'];
    function Harness() {
      select = useCanvasOverlayHandlers({
        canvasElementRef: { current: canvas },
        groupId,
        activeTabId: graphPath,
        pendingConnection: pin(),
        setContextMenu,
        setPendingConnection,
      }).handleNodePaletteSelect;
      return null;
    }
    act(() => root.render(<Harness />));
    useEditorStore.setState({
      contextMenu: { x: 20, y: 30, visible: true },
      pendingConnection: pin(),
    });

    const selection = select(
      { kind: 'static', nodeTypeId: 'math.add' },
      'en-US',
      { x: 20, y: 30 },
    );
    useEditorStore.setState({
      contextMenu: { x: 40, y: 50, visible: true },
      pendingConnection: pin(newerSourceAddress),
    });
    pending.resolve({ status: 'applied', result: {} as never });
    await act(async () => selection);

    expect(setContextMenu).not.toHaveBeenCalled();
    expect(setPendingConnection).not.toHaveBeenCalled();
    expect(useEditorStore.getState()).toMatchObject({
      contextMenu: { x: 40, y: 50, visible: true },
      pendingConnection: { address: newerSourceAddress },
    });
  });

  it('handles descriptor creation rejection with actionable feedback', async () => {
    vi.mocked(createNodeFromDescriptor).mockRejectedValueOnce(new Error('mutation transport unavailable'));
    let select!: ReturnType<typeof useCanvasOverlayHandlers>['handleNodePaletteSelect'];
    function Harness() {
      select = useCanvasOverlayHandlers({
        canvasElementRef: { current: canvas },
        groupId,
        activeTabId: graphPath,
        pendingConnection: null,
        setContextMenu: vi.fn(),
        setPendingConnection: vi.fn(),
      }).handleNodePaletteSelect;
      return null;
    }
    act(() => root.render(<Harness />));

    await expect(select(
      { kind: 'static', nodeTypeId: 'math.add' },
      'en-US',
      { x: 20, y: 30 },
    )).resolves.toBeUndefined();

    expect(logError).toHaveBeenCalledWith(
      "Failed to create node 'math.add' in 'events/main.yssbi-event': mutation transport unavailable",
      'NodePalette',
    );
    expect(showToast).toHaveBeenCalledWith(
      'Failed to create node: mutation transport unavailable',
      'error',
      4000,
    );
  });

  it('keeps coordinator stale recovery as a resolved non-error outcome', async () => {
    vi.mocked(createNodeFromDescriptor).mockResolvedValueOnce({
      status: 'stale',
      result: {
        projectInstanceId: 'project-1',
        delta: {
          graphPath,
          fromRevision: 1,
          toRevision: 2,
          causedBy: 'stale-operation',
          payload: { operations: [] },
        },
        projectionReplacement: {
          graphPath,
          projection: makeEditorProjectionFixture({
            graphPath,
            sourceRevision: 2,
            title: 'Stale projection',
          }).projection,
        },
        history: { canUndo: true, canRedo: false },
      },
    });
    let select!: ReturnType<typeof useCanvasOverlayHandlers>['handleNodePaletteSelect'];
    function Harness() {
      select = useCanvasOverlayHandlers({
        canvasElementRef: { current: canvas },
        groupId,
        activeTabId: graphPath,
        pendingConnection: null,
        setContextMenu: vi.fn(),
        setPendingConnection: vi.fn(),
      }).handleNodePaletteSelect;
      return null;
    }
    act(() => root.render(<Harness />));

    await expect(select(
      { kind: 'static', nodeTypeId: 'math.add' },
      'en-US',
      { x: 20, y: 30 },
    )).resolves.toBeUndefined();

    expect(logError).not.toHaveBeenCalled();
    expect(showToast).not.toHaveBeenCalled();
  });

  it('routes resource-bound palette descriptors unchanged', async () => {
    let select!: ReturnType<typeof useCanvasOverlayHandlers>['handleNodePaletteSelect'];
    function Harness() {
      select = useCanvasOverlayHandlers({
        canvasElementRef: { current: canvas },
        groupId,
        activeTabId: graphPath,
        pendingConnection: null,
        setContextMenu: vi.fn(),
        setPendingConnection: vi.fn(),
      }).handleNodePaletteSelect;
      return null;
    }
    act(() => root.render(<Harness />));

    await act(async () => {
      await select(
        resourceDescriptor,
        'en-US',
        { x: 20, y: 30 },
      );
    });

    expect(createNodeFromDescriptor).toHaveBeenCalledWith(expect.objectContaining({
      descriptor: resourceDescriptor,
    }));
    expect(showToast).not.toHaveBeenCalled();
  });

  it('shift-drops a function only through its exact current opaque Catalog path', async () => {
    let routeDrop!: ReturnType<typeof useCanvasDrop>['handleSidebarCanvasDrop'];
    function Harness() {
      routeDrop = useCanvasDrop({
        canvasElementRef: { current: canvas }, groupId, graphPath,
        variables: {}, functions: {}, setContextMenu: vi.fn(), setPendingConnection: vi.fn(),
        createNode, enabled: false,
      }).handleSidebarCanvasDrop;
      return null;
    }
    act(() => root.render(<Harness />));
    createNode.mockResolvedValueOnce(true);

    await expect(routeDrop({
      type: 'graph-resource',
      sidebarResource: { id: resourceDescriptor.resourcePath, name: 'Helper', type: 'function' },
      x: 20,
      y: 30,
    }, { altKey: false, ctrlKey: false, shiftKey: true })).resolves.toBe(true);

    expect(createNode).toHaveBeenCalledWith(resourceDescriptor, { x: 20, y: 30 });
    expect(refreshCatalog).not.toHaveBeenCalled();
  });

  it('rejects a shift-drop without an exact path, refreshes, and toasts without synthesis', async () => {
    let routeDrop!: ReturnType<typeof useCanvasDrop>['handleSidebarCanvasDrop'];
    function Harness() {
      routeDrop = useCanvasDrop({
        canvasElementRef: { current: canvas }, groupId, graphPath,
        variables: {}, functions: {}, setContextMenu: vi.fn(), setPendingConnection: vi.fn(),
        createNode, enabled: false,
      }).handleSidebarCanvasDrop;
      return null;
    }
    act(() => root.render(<Harness />));

    await expect(routeDrop({
      type: 'graph-resource',
      sidebarResource: { id: 'functions/helper', name: 'Helper', type: 'function' },
      x: 20,
      y: 30,
    }, { altKey: false, ctrlKey: false, shiftKey: true })).resolves.toBe(false);

    expect(createNode).not.toHaveBeenCalled();
    expect(refreshCatalog).toHaveBeenCalledOnce();
    expect(showToast).toHaveBeenCalledWith(
      'Resource catalog is stale. Refreshing before node creation.',
      'warning',
    );
  });

  it('forwards sidebar node-template descriptors unchanged', async () => {
    let routeDrop!: ReturnType<typeof useCanvasDrop>['handleSidebarCanvasDrop'];
    function Harness() {
      routeDrop = useCanvasDrop({
        canvasElementRef: { current: canvas },
        groupId,
        graphPath,
        variables: {},
        functions: {},
        setContextMenu: vi.fn(),
        setPendingConnection: vi.fn(),
        createNode,
        enabled: false,
      }).handleSidebarCanvasDrop;
      return null;
    }
    act(() => root.render(<Harness />));

    createNode.mockResolvedValueOnce(true);
    const descriptor: NodeCreationDescriptor = { kind: 'static', nodeTypeId: 'math.add' };
    await expect(routeDrop({
      type: 'node-template',
      template: { title: 'Add', descriptor },
      x: 20,
      y: 30,
    }, { altKey: false, ctrlKey: false, shiftKey: false })).resolves.toBe(true);

    expect(createNode).toHaveBeenCalledWith(descriptor, { x: 20, y: 30 });
    expect(showToast).not.toHaveBeenCalled();
  });
});
