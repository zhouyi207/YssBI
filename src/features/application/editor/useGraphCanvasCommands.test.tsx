// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useEditorStore } from '@/features/core/editor';
import { useGraphCanvasCommands } from './useGraphCanvasCommands';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  activeGroupId: 'group-a' as string | null,
  activeTab: { activeTabId: 'events/main.yssbi-event', tab: { type: 'event' } } as {
    activeTabId: string;
    tab: { type: string };
  } | null,
  selectedNodeIds: ['node-b'] as string[],
  graphNodes: ['node-a', 'managed', 'node-b'] as string[],
  nodes: {
    'node-a': { capabilities: { managed: false } },
    managed: { capabilities: { managed: true } },
    'node-b': { capabilities: { managed: false } },
  } as Record<string, { capabilities: { managed: boolean } }>,
  documentLoaded: true,
  projectionAvailable: true,
  updateSelectedNodeIds: vi.fn(),
  setViewportLive: vi.fn(),
  commitViewport: vi.fn(),
  persistGraphViewport: vi.fn(),
  getViewport: vi.fn(() => ({ x: 0, y: 0, scale: 1 })),
}));

vi.mock('@/features/core/dockview', () => ({
  editorDockviewPort: { getActiveGroupId: () => mocks.activeGroupId },
}));
vi.mock('@/features/core/layout/layoutTabQueries', () => ({
  getActiveLayoutTab: () => mocks.activeTab,
  getEditorGroupGraphSelection: () => ({
    nodeIds: new Set(mocks.selectedNodeIds),
    connectionIds: new Set(),
  }),
  updateEditorGroupSelectedNodeIds: mocks.updateSelectedNodeIds,
}));
vi.mock('@/features/core/dataStore', () => ({
  useGraphDataStore: {
    getState: () => ({
      graphEntities: mocks.activeTab && mocks.projectionAvailable
        ? { [mocks.activeTab.activeTabId]: { graphNodes: mocks.graphNodes, nodes: mocks.nodes } }
        : {},
    }),
  },
}));
vi.mock('@/features/core/resource', () => ({
  getDocumentState: () => ({ loaded: mocks.documentLoaded }),
}));
vi.mock('@/features/core/viewport', () => ({
  editorViewportScope: (groupId: string, graphPath: string) => ({ groupId, graphPath }),
  getViewport: mocks.getViewport,
  setViewportLive: mocks.setViewportLive,
  commitViewport: mocks.commitViewport,
  persistGraphViewport: mocks.persistGraphViewport,
  fitWorldBounds: (bounds: unknown, size: unknown) => ({ x: 11, y: 22, scale: 3, bounds, size }),
}));

let root: Root;
let commands: ReturnType<typeof useGraphCanvasCommands>;

function setRect(element: HTMLElement, rect: Partial<DOMRect>): void {
  element.getBoundingClientRect = () => ({
    x: rect.left ?? 0,
    y: rect.top ?? 0,
    left: rect.left ?? 0,
    top: rect.top ?? 0,
    right: rect.right ?? 0,
    bottom: rect.bottom ?? 0,
    width: rect.width ?? (rect.right ?? 0) - (rect.left ?? 0),
    height: rect.height ?? (rect.bottom ?? 0) - (rect.top ?? 0),
    toJSON: () => ({}),
  });
}

function installCanvas(groupId: string, offset = 0): HTMLElement {
  const canvas = document.createElement('div');
  canvas.dataset.editorGroupId = groupId;
  setRect(canvas, { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600 });
  for (const [id, left] of [['node-a', 10], ['managed', 100], ['node-b', 200]] as const) {
    const node = document.createElement('div');
    node.dataset.nodeId = id;
    setRect(node, { left: left + offset, top: 20, right: left + offset + 50, bottom: 70 });
    canvas.appendChild(node);
  }
  document.body.appendChild(canvas);
  return canvas;
}

describe('useGraphCanvasCommands', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.updateSelectedNodeIds.mockReset();
    mocks.updateSelectedNodeIds.mockReturnValue({
      groupId: 'group-a',
      nodeIds: ['node-a', 'node-b'],
    });
    useEditorStore.setState({ detailFocus: null, rightSidebarTab: 'details' });
    document.body.replaceChildren();
    mocks.activeGroupId = 'group-a';
    mocks.activeTab = { activeTabId: 'events/main.yssbi-event', tab: { type: 'event' } };
    mocks.selectedNodeIds = ['node-b'];
    mocks.documentLoaded = true;
    mocks.projectionAvailable = true;
    mocks.graphNodes = ['node-a', 'managed', 'node-b'];
    installCanvas('group-a');
    installCanvas('group-b', 1000);
    root = createRoot(document.createElement('div'));
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
  });

  function Harness() {
    commands = useGraphCanvasCommands();
    return null;
  }

  it('returns true after selecting eligible nodes in graph order', () => {
    expect(commands.selectAllNodes()).toBe(true);
    expect(mocks.updateSelectedNodeIds).toHaveBeenCalledWith(['node-a', 'node-b'], 'group-a');
  });

  it('returns false without focusing Inspect when the selection write cannot settle', () => {
    mocks.updateSelectedNodeIds.mockReturnValueOnce(null);
    useEditorStore.setState({
      rightSidebarTab: 'result',
      detailFocus: {
        kind: 'node',
        id: 'stale-node',
        graphPath: 'events/main.yssbi-event',
      },
    });

    expect(commands.selectAllNodes()).toBe(false);
    expect(useEditorStore.getState()).toMatchObject({
      rightSidebarTab: 'result',
      detailFocus: {
        kind: 'node',
        id: 'stale-node',
        graphPath: 'events/main.yssbi-event',
      },
    });
  });

  it.each([
    ['no active graph', () => { mocks.activeTab = null; }],
    ['an unloaded graph', () => { mocks.documentLoaded = false; }],
    ['no projection', () => { mocks.projectionAvailable = false; }],
    ['no eligible nodes', () => { mocks.graphNodes = ['managed']; }],
  ])('returns false and leaves selection unchanged for %s', (_label, arrange) => {
    arrange();
    expect(commands.selectAllNodes()).toBe(false);
    expect(mocks.updateSelectedNodeIds).not.toHaveBeenCalled();
  });

  it('focuses only selected nodes in the active group and persists without IPC or history', () => {
    expect(commands.focusSelectedNodes()).toBe(true);

    expect(mocks.setViewportLive).toHaveBeenCalledWith(
      { groupId: 'group-a', graphPath: 'events/main.yssbi-event' },
      expect.objectContaining({ x: 11, y: 22, scale: 3 }),
    );
    expect(mocks.setViewportLive.mock.calls[0][1].bounds).toEqual({
      left: 200, top: 20, right: 250, bottom: 70,
    });
    expect(mocks.commitViewport).toHaveBeenCalledWith({
      groupId: 'group-a', graphPath: 'events/main.yssbi-event',
    });
    expect(mocks.persistGraphViewport).toHaveBeenCalledWith({
      groupId: 'group-a', graphPath: 'events/main.yssbi-event',
    });
  });

  it('returns false for focus with an empty selection or no matching bounds', () => {
    mocks.selectedNodeIds = [];
    expect(commands.focusSelectedNodes()).toBe(false);

    mocks.selectedNodeIds = ['missing'];
    expect(commands.focusSelectedNodes()).toBe(false);
    expect(mocks.setViewportLive).not.toHaveBeenCalled();
  });

  it('fits all rendered nodes for Home and affects only the active editor group', () => {
    expect(commands.fitCompleteGraph()).toBe(true);
    const viewport = mocks.setViewportLive.mock.calls[0][1];
    expect(viewport.bounds).toEqual({ left: 10, top: 20, right: 250, bottom: 70 });
    expect(viewport.size).toEqual({ width: 800, height: 600 });
  });

  it('returns false for Home when the active canvas has no node bounds', () => {
    document.querySelector<HTMLElement>('[data-editor-group-id="group-a"]')?.replaceChildren();
    expect(commands.fitCompleteGraph()).toBe(false);
    expect(mocks.setViewportLive).not.toHaveBeenCalled();
  });
});
