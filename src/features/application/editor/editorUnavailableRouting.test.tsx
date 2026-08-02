// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { uiStore } from '@/features/core/ui/UIStore';
import { logger } from '@/utils/appLogger';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import type { Pin } from '@/shared/types/domain/pin';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import { createNodeFromDescriptor } from '@/features/application/nodeCatalog/createNodeFromDescriptor';
import { useCanvasDrop } from './useCanvasDrop';
import { EDITOR_MUTATION_CAPABILITIES } from './editorMutationAvailability';
import { useCanvasOverlayHandlers } from './useCanvasOverlayHandlers';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const executeCommand = vi.hoisted(() => vi.fn());
vi.mock('@/features/core/history', () => ({ executeCommand }));
vi.mock('@/features/application/nodeCatalog/createNodeFromDescriptor', () => ({
  createNodeFromDescriptor: vi.fn().mockResolvedValue({ status: 'conflict' }),
}));



const graphPath = 'events/main.yssbi-event';
const groupId = 'group-1';

function pin(): Pin {
  return {
    id: 'output',
    nodeId: 'source',
    name: 'Output',
    direction: 'output',
    type: 'object',
  } as Pin;
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
    });
    expect(createNode).not.toHaveBeenCalled();
    expect(executeCommand).not.toHaveBeenCalled();
    expect(showToast).not.toHaveBeenCalled();
    expect(setContextMenu).toHaveBeenCalledWith(null);
    expect(setPendingConnection).toHaveBeenCalledWith(null);
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

  it('keeps resource-bound palette descriptors unavailable', async () => {
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
        {
          kind: 'resourceBound',
          nodeTypeId: 'functions.call',
          resourcePath: 'functions/helper.yssbi-function',
        } as unknown as NodeCreationDescriptor,
        'en-US',
        { x: 20, y: 30 },
      );
    });

    expect(createNodeFromDescriptor).not.toHaveBeenCalled();
    expect(showToast).toHaveBeenCalled();
  });

  it('exposes only static catalog descriptor creation capability', () => {
    expect(EDITOR_MUTATION_CAPABILITIES).toEqual({
      createStaticNodes: true,
      catalogDescriptors: true,
      resourceBoundDescriptors: false,
      contextualCompatibility: false,
      nodeDocumentation: false,
      duplicateNodes: false,
      pasteNodes: false,
    });
  });

  it('rejects sidebar node-template drops before invoking createNode', async () => {
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

    await expect(routeDrop({
      type: 'node-template',
      template: { nodeType: 'Math:Add', title: 'Add' },
      x: 20,
      y: 30,
    }, { altKey: false, ctrlKey: false, shiftKey: false })).resolves.toBe(false);

    expect(createNode).not.toHaveBeenCalled();
    expect(showToast).toHaveBeenCalled();
  });
});
