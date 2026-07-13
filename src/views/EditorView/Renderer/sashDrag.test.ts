// @vitest-environment happy-dom

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { attachSashDrag, restoreAdjacentPanelVisibility } from './sashResizeLogic';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { enterZenMode } from '@/features/core/layout/workbenchZenMode';
import {
  createInitialWorkbenchNodes,
  DEFAULT_EDITOR_GROUP_ID,
  EDITOR_AREA_ID,
  PANEL_PART_ID,
} from '@/features/core/layout/workbenchLayoutDefaults';

vi.mock('@/features/core/layout/workbenchLayoutService', () => ({
  persistWorkbenchLayoutDebounced: vi.fn(),
  persistEditorGridDebounced: vi.fn(),
  togglePanelMaximized: vi.fn(),
}));

function mockRect(width: number, height: number): DOMRect {
  return {
    width,
    height,
    top: 0,
    left: 0,
    right: width,
    bottom: height,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  } as DOMRect;
}

function layoutDiv(width: number, height: number): HTMLDivElement {
  const el = document.createElement('div');
  el.getBoundingClientRect = () => mockRect(width, height);
  return el;
}

describe('attachSashDrag store commits', () => {
  let resizeSpy: ReturnType<typeof vi.spyOn>;
  let detach: (() => void) | null = null;

  beforeEach(() => {
    useLayoutStore.setState({
      nodes: {
        sidebar: {
          id: 'sidebar',
          type: 'component',
          parentId: 'root',
          pixelSize: 260,
          minSize: 240,
          data: { component: 'Sidebar', visible: true },
        },
        center: {
          id: 'center',
          type: 'col',
          parentId: 'root',
          size: 1,
          children: [],
        },
      },
    });

    resizeSpy = vi.spyOn(useLayoutStore.getState(), 'resizeNode');
  });

  afterEach(() => {
    detach?.();
    detach = null;
    resizeSpy.mockRestore();
    vi.clearAllMocks();
  });

  it('commits resizeNode once on mouseup; no store writes during mousemove', () => {
    const beforeEl = layoutDiv(260, 600);
    const afterEl = layoutDiv(800, 600);
    const sash = document.createElement('div');

    detach = attachSashDrag(sash, {
      orientation: 'row',
      beforeNodeId: 'sidebar',
      afterNodeId: 'center',
      getBeforeEl: () => beforeEl,
      getAfterEl: () => afterEl,
    });

    sash.dispatchEvent(new MouseEvent('mousedown', { clientX: 100, clientY: 300, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 120, clientY: 300, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 140, clientY: 300, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 160, clientY: 300, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mouseup', { clientX: 160, clientY: 300, bubbles: true }));

    expect(resizeSpy).toHaveBeenCalledTimes(1);
    expect(resizeSpy).toHaveBeenCalledWith('sidebar', 320);
  });

  it('does not restore adjacent panel visibility while Zen is active', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes.panel!.data = { ...nodes.panel!.data, visible: false };
    useLayoutStore.setState({ nodes });
    enterZenMode();
    const updateSpy = vi.spyOn(useLayoutStore.getState(), 'updateNode');

    restoreAdjacentPanelVisibility(EDITOR_AREA_ID, PANEL_PART_ID);

    expect(updateSpy).not.toHaveBeenCalled();
    expect(useLayoutStore.getState().nodes.panel?.data?.visible).toBe(false);
    updateSpy.mockRestore();
  });

  it('does not restore a detail part that the user explicitly hid', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes.detail!.data = { ...nodes.detail!.data, visible: false, userHidden: true };
    useLayoutStore.setState({ nodes });
    const updateSpy = vi.spyOn(useLayoutStore.getState(), 'updateNode');

    restoreAdjacentPanelVisibility('center', 'detail');

    expect(updateSpy).not.toHaveBeenCalled();
    expect(useLayoutStore.getState().nodes.detail?.data?.visible).toBe(false);
    updateSpy.mockRestore();
  });

  it('does not restore a sidebar that the user explicitly hid', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes.sidebar!.data = { ...nodes.sidebar!.data, visible: false, userHidden: true };
    useLayoutStore.setState({ nodes });
    const updateSpy = vi.spyOn(useLayoutStore.getState(), 'updateNode');

    restoreAdjacentPanelVisibility('sidebar', 'center');

    expect(updateSpy).not.toHaveBeenCalled();
    expect(useLayoutStore.getState().nodes.sidebar?.data?.visible).toBe(false);
    updateSpy.mockRestore();
  });

  it.each([
    {
      position: 'left',
      children: [PANEL_PART_ID, EDITOR_AREA_ID],
      beforeNodeId: PANEL_PART_ID,
      afterNodeId: EDITOR_AREA_ID,
      startX: 100,
      endX: 1_100,
    },
    {
      position: 'right',
      children: [EDITOR_AREA_ID, PANEL_PART_ID],
      beforeNodeId: EDITOR_AREA_ID,
      afterNodeId: PANEL_PART_ID,
      startX: 1_100,
      endX: 100,
    },
  ])('commits the $position panel width using the horizontal viewport clamp', ({
    children,
    beforeNodeId,
    afterNodeId,
    startX,
    endX,
  }) => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1_200 });
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 1_000 });

    const nodes = createInitialWorkbenchNodes();
    nodes.center!.type = 'row';
    nodes.center!.children = children;
    useLayoutStore.setState({ nodes });

    const panelEl = layoutDiv(200, 1_000);
    const editorEl = layoutDiv(1_000, 1_000);
    const sash = document.createElement('div');
    detach = attachSashDrag(sash, {
      orientation: 'row',
      beforeNodeId,
      afterNodeId,
      getBeforeEl: () => beforeNodeId === PANEL_PART_ID ? panelEl : editorEl,
      getAfterEl: () => afterNodeId === PANEL_PART_ID ? panelEl : editorEl,
    });

    sash.dispatchEvent(new MouseEvent('mousedown', { clientX: startX, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: endX, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mouseup', { clientX: endX, bubbles: true }));

    expect(useLayoutStore.getState().nodes[PANEL_PART_ID]?.pixelSize).toBe(960);
  });

  it('resizes both editor groups on a second sash drag after the first drag pixelizes them', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.type = 'row';
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
    nodes[DEFAULT_EDITOR_GROUP_ID]!.size = 1;
    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = undefined;
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 1,
      data: { component: 'GraphEditor' },
    };
    useLayoutStore.setState({ nodes });

    let beforeWidth = 500;
    let afterWidth = 500;
    const beforeEl = layoutDiv(beforeWidth, 600);
    const afterEl = layoutDiv(afterWidth, 600);
    beforeEl.getBoundingClientRect = () => mockRect(beforeWidth, 600);
    afterEl.getBoundingClientRect = () => mockRect(afterWidth, 600);
    const sash = document.createElement('div');
    detach = attachSashDrag(sash, {
      orientation: 'row',
      beforeNodeId: DEFAULT_EDITOR_GROUP_ID,
      afterNodeId: 'editor_group_2',
      getBeforeEl: () => beforeEl,
      getAfterEl: () => afterEl,
    });

    sash.dispatchEvent(new MouseEvent('mousedown', { clientX: 100, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 200, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mouseup', { clientX: 200, bubbles: true }));
    expect(useLayoutStore.getState().nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBe(600);
    expect(useLayoutStore.getState().nodes.editor_group_2?.pixelSize).toBe(400);

    beforeWidth = 600;
    afterWidth = 400;
    sash.dispatchEvent(new MouseEvent('mousedown', { clientX: 200, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 250, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mouseup', { clientX: 250, bubbles: true }));

    expect(useLayoutStore.getState().nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBe(650);
    expect(useLayoutStore.getState().nodes.editor_group_2?.pixelSize).toBe(350);
  });
});
