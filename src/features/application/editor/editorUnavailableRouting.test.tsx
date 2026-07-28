// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { uiStore } from '@/features/core/ui/UIStore';
import type { Pin } from '@/shared/types/domain/pin';
import { useCanvasDrop } from './useCanvasDrop';
import { useCanvasOverlayHandlers } from './useCanvasOverlayHandlers';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const executeCommand = vi.hoisted(() => vi.fn());
vi.mock('@/features/core/history', () => ({ executeCommand }));


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
  ])('does not invoke unavailable commands for $name', async ({ pendingConnection }) => {
    let select!: ReturnType<typeof useCanvasOverlayHandlers>['handleNodePaletteSelect'];
    function Harness() {
      select = useCanvasOverlayHandlers({
        canvasElementRef: { current: canvas },
        groupId,
        activeTabId: graphPath,
        functions: {},
        pendingConnection,
        setContextMenu: vi.fn(),
        setPendingConnection: vi.fn(),
        createNode,
        setCanvas: vi.fn(),
      }).handleNodePaletteSelect;
      return null;
    }
    act(() => root.render(<Harness />));

    await act(async () => {
      await select(
        { nodeType: 'Math:Add', title: 'Add', category: ['Math'] },
        { x: 20, y: 30 },
      );
    });

    expect(createNode).not.toHaveBeenCalled();
    expect(executeCommand).not.toHaveBeenCalled();
    expect(showToast).toHaveBeenCalled();
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
