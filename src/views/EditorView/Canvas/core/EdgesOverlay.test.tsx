// @vitest-environment happy-dom

import React, { act, createRef, useState } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import {
  updateEditorGroupSelectedConnectionIds,
  updateEditorGroupSelectedNodeIds,
} from '@/features/core/layout/layoutTabQueries';
import {
  useCanvasInteraction,
  type CanvasInteractionHandlers,
} from '@/features/core/canvas/useCanvasInteraction';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import {
  buildEdgeData,
  EdgesOverlay,
  replacementEdgeAttributes,
} from './EdgesOverlay';

vi.mock('@/features/core/canvas/useEdgeDragPreview', () => ({ useEdgeDragPreview: vi.fn() }));
vi.mock('react-i18next', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-i18next')>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let portal: HTMLDivElement;
let root: Root;
let doubleClickCompletion: Promise<unknown> | null;
let insertRerouteCommand: ReturnType<
  typeof vi.fn<CanvasInteractionHandlers['insertRerouteAtConnection']>
>;

beforeEach(() => {
  useGraphDataStore.setState({ graphEntities: {} });
  useEditorTabStore.setState({
    registry: {
      'graph-a': { id: 'graph-a', component: 'GraphEditor', type: 'event' },
    },
    placements: {
      'group-a': {
        tabIds: ['graph-a'],
        activeTabId: 'graph-a',
        selectedNodeIds: ['node-a'],
        selectedConnectionIds: [],
        selectedTabIds: ['graph-a'],
      },
    },
  });
  const fixture = makeEditorProjectionFixture({ graphPath: 'graph-a', connectionId: 'edge-a' });
  useGraphDataStore.getState().replaceProjection('graph-a', fixture.projection, 1);
  container = document.createElement('div');
  portal = document.createElement('div');
  portal.id = 'portal';
  document.body.append(container, portal);
  root = createRoot(container);
  doubleClickCompletion = null;
  insertRerouteCommand = vi.fn<CanvasInteractionHandlers['insertRerouteAtConnection']>();
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  portal.remove();
});

describe('EdgesOverlay', () => {
  it('marks only advisory replacement IDs without edge interaction metadata', () => {
    expect(replacementEdgeAttributes('connection-a', new Set(['connection-a']))).toEqual({
      'data-replacement-preview': true,
    });
    expect(replacementEdgeAttributes('connection-b', new Set(['connection-a']))).toEqual({});
  });

  it('renders edges from the requested projected graph when local ids overlap', () => {
    const first = makeEditorProjectionFixture({ graphPath: 'graph-1' });
    const second = makeEditorProjectionFixture({ graphPath: 'graph-2' });
    const store = useGraphDataStore.getState();
    store.replaceProjection('graph-1', first.projection, 1);
    store.replaceProjection('graph-2', second.projection, 1);

    const edges = buildEdgeData(
      store.getGraphNodeIds('graph-1'),
      store.getGraphConnections('graph-1'),
      (pinId) => store.getGraphPin('graph-1', pinId),
    );

    expect(edges).toEqual([
      expect.objectContaining({
        id: 'local-connection',
        fromPinId: first.outputKey,
        toPinId: first.inputKey,
        sourceNodeId: 'local-node',
      }),
    ]);
  });

  it('selects and toggles edges with graph and group scope while stopping canvas gestures', () => {
    const canvasPointerDown = vi.fn();
    const selectionChanges: Array<{ ids: string[]; graphPath: string; groupId: string }> = [];
    renderInteractive({
      onCanvasPointerDown: canvasPointerDown,
      onSelectionChange: (ids, graphPath, groupId) => selectionChanges.push({ ids, graphPath, groupId }),
    });
    const hit = container.querySelector('[data-edge-hit-target="edge-a"]')!;

    const pointer = new PointerEvent('pointerdown', { bubbles: true, cancelable: true, button: 0 });
    act(() => hit.dispatchEvent(pointer));
    expect(selectionChanges).toEqual([]);

    const ordinary = new MouseEvent('click', { bubbles: true, cancelable: true, button: 0, detail: 1 });
    act(() => hit.dispatchEvent(ordinary));
    expect(pointer.defaultPrevented).toBe(true);
    expect(ordinary.defaultPrevented).toBe(true);
    expect(canvasPointerDown).not.toHaveBeenCalled();
    expect(selectionChanges[selectionChanges.length - 1]).toEqual({ ids: ['edge-a'], graphPath: 'graph-a', groupId: 'group-a' });

    act(() => hit.dispatchEvent(new MouseEvent('click', {
      bubbles: true,
      cancelable: true,
      button: 0,
      detail: 2,
      ctrlKey: true,
    })));
    expect(selectionChanges).toHaveLength(1);

    act(() => hit.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0, detail: 1, ctrlKey: true })));
    expect(selectionChanges[selectionChanges.length - 1]).toEqual({ ids: [], graphPath: 'graph-a', groupId: 'group-a' });

    act(() => hit.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0, detail: 1, shiftKey: true })));
    expect(selectionChanges[selectionChanges.length - 1]).toEqual({ ids: ['edge-a'], graphPath: 'graph-a', groupId: 'group-a' });

    act(() => hit.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0, detail: 1, metaKey: true })));
    expect(selectionChanges[selectionChanges.length - 1]).toEqual({ ids: [], graphPath: 'graph-a', groupId: 'group-a' });
  });

  it('uses the scoped selection authority to make edge and node selection mutually exclusive', () => {
    renderInteractive({
      onSelectionChange: (ids, graphPath, groupId) => {
        expect(graphPath).toBe('graph-a');
        updateEditorGroupSelectedConnectionIds(ids, groupId);
      },
    });

    const hit = container.querySelector('[data-edge-hit-target="edge-a"]')!;
    act(() => hit.dispatchEvent(new MouseEvent('click', {
      bubbles: true,
      cancelable: true,
      button: 0,
      detail: 1,
    })));

    expect(useEditorTabStore.getState().getPlacement('group-a')).toEqual(expect.objectContaining({
      selectedNodeIds: [],
      selectedConnectionIds: ['edge-a'],
    }));
  });

  it('right-clicking an unselected edge replaces the active selection with that edge', () => {
    const selectionChanges: string[][] = [];
    renderInteractive({
      initialSelection: ['edge-b', 'edge-c'],
      onSelectionChange: (ids) => selectionChanges.push(ids),
    });
    const hit = container.querySelector('[data-edge-hit-target="edge-a"]')!;

    act(() => hit.dispatchEvent(new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      button: 2,
    })));

    expect(selectionChanges).toEqual([['edge-a']]);
    expect(portal.textContent).toContain('contextMenu.connection.breakLink');
  });

  it('right-click selects an unselected edge, retains a selected set, and invokes one scoped break command', () => {
    const onBreakConnections = vi.fn(() => false);
    renderInteractive({ initialSelection: ['edge-a', 'edge-b'], onBreakConnections });
    const hit = container.querySelector('[data-edge-hit-target="edge-a"]')!;

    act(() => hit.dispatchEvent(new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      button: 2,
      clientX: 40,
      clientY: 50,
    })));
    expect(portal.querySelectorAll('[role="menu"]')).toHaveLength(1);

    const item = portal.querySelector('[role="menuitem"]')!;
    act(() => item.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 })));
    expect(onBreakConnections).toHaveBeenCalledTimes(1);
    expect(onBreakConnections).toHaveBeenCalledWith(
      ['edge-a', 'edge-b'],
      'graph-a',
      'group-a',
    );
  });

  it('preserves the pre-double-click graph selection across click detail one and two', () => {
    const canvasDoubleClick = vi.fn();
    const getCanvasLocalPoint = vi.fn(() => ({ x: 25, y: 40 }));
    const onEdgeDoubleClick = vi.fn();
    renderInteractive({
      initialNodeSelection: [],
      initialSelection: ['edge-b'],
      onCanvasDoubleClick: canvasDoubleClick,
      getCanvasLocalPoint,
      onEdgeDoubleClick,
    });
    const hit = container.querySelector('[data-edge-hit-target="edge-a"]')!;
    act(() => hit.dispatchEvent(new MouseEvent('click', {
      bubbles: true,
      cancelable: true,
      button: 0,
      detail: 1,
      clientX: 125,
      clientY: 240,
    })));
    act(() => hit.dispatchEvent(new MouseEvent('click', {
      bubbles: true,
      cancelable: true,
      button: 0,
      detail: 2,
      clientX: 126,
      clientY: 241,
    })));
    const event = new MouseEvent('dblclick', {
      bubbles: true,
      cancelable: true,
      button: 0,
      detail: 2,
      clientX: 125,
      clientY: 240,
    });

    act(() => hit.dispatchEvent(event));

    expect(event.defaultPrevented).toBe(true);
    expect(canvasDoubleClick).not.toHaveBeenCalled();
    expect(getCanvasLocalPoint).toHaveBeenCalledOnce();
    expect(getCanvasLocalPoint).toHaveBeenCalledWith(125, 240);
    expect(onEdgeDoubleClick).toHaveBeenCalledOnce();
    expect(onEdgeDoubleClick).toHaveBeenCalledWith(
      'edge-a',
      { x: 25, y: 40 },
      'graph-a',
      'group-a',
      {
        before: { nodeIds: new Set(), connectionIds: new Set(['edge-b']) },
        temporary: { nodeIds: new Set(), connectionIds: new Set(['edge-a']) },
      },
    );
  });

  it('restores edge-b after click detail one and two then failed double click', async () => {
    insertRerouteCommand.mockResolvedValueOnce({ status: 'failed' });
    renderDoubleClickIntegration(['edge-b']);

    dispatchDoubleClickSequence();
    await act(async () => doubleClickCompletion);

    expect(insertRerouteCommand).toHaveBeenCalledTimes(1);
    expect(useEditorTabStore.getState().getPlacement('group-a').selectedConnectionIds).toEqual(['edge-b']);
  });

  it('clears edge-a after click detail one and two then applied double click', async () => {
    insertRerouteCommand.mockResolvedValueOnce({ status: 'applied' });
    renderDoubleClickIntegration(['edge-b']);

    dispatchDoubleClickSequence();
    await act(async () => doubleClickCompletion);

    expect(insertRerouteCommand).toHaveBeenCalledTimes(1);
    expect(useEditorTabStore.getState().getPlacement('group-a').selectedConnectionIds).toEqual([]);
  });

  it('overwrites an old standalone click snapshot on the next independent detail-one click', async () => {
    insertRerouteCommand.mockResolvedValueOnce({ status: 'failed' });
    renderDoubleClickIntegration(['edge-original']);
    const hit = container.querySelector('[data-edge-hit-target="edge-a"]')!;

    act(() => hit.dispatchEvent(new MouseEvent('click', { bubbles: true, button: 0, detail: 1 })));
    act(() => updateEditorGroupSelectedConnectionIds(['edge-new-before'], 'group-a'));
    dispatchDoubleClickSequence();
    await act(async () => doubleClickCompletion);

    expect(insertRerouteCommand).toHaveBeenCalledTimes(1);
    expect(useEditorTabStore.getState().getPlacement('group-a').selectedConnectionIds).toEqual(['edge-new-before']);
  });

  it('replaces the pending snapshot when a different edge receives an independent detail-one click', async () => {
    installSecondProjectedEdge();
    insertRerouteCommand.mockResolvedValueOnce({ status: 'failed' });
    renderDoubleClickIntegration(['edge-original']);

    dispatchDoubleClickSequence({
      secondConnectionId: 'edge-b',
      secondDetail: 1,
      doubleClickConnectionId: 'edge-b',
    });
    await act(async () => doubleClickCompletion);

    expect(insertRerouteCommand).toHaveBeenCalledTimes(1);
    expect(useEditorTabStore.getState().getPlacement('group-a').selectedConnectionIds).toEqual(['edge-a']);
  });

  it('preserves a user selection changed while the real double click command is pending', async () => {
    let resolve!: (value: { status: 'failed' }) => void;
    insertRerouteCommand.mockReturnValueOnce(new Promise((settle) => { resolve = settle; }));
    renderDoubleClickIntegration(['edge-b']);

    dispatchDoubleClickSequence();
    expect(insertRerouteCommand).toHaveBeenCalledTimes(1);
    act(() => updateEditorGroupSelectedConnectionIds(['edge-user'], 'group-a'));
    await act(async () => {
      resolve({ status: 'failed' });
      await Promise.resolve();
    });

    expect(insertRerouteCommand).toHaveBeenCalledTimes(1);
    expect(useEditorTabStore.getState().getPlacement('group-a').selectedConnectionIds).toEqual(['edge-user']);
  });

  it('clears hidden menu state after StrictMode interaction transitions without render updates', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const rerender = renderInteractive({}, true);
    const hit = container.querySelector('[data-edge-hit-target="edge-a"]')!;
    act(() => hit.dispatchEvent(new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      button: 2,
    })));
    expect(portal.querySelector('[role="menu"]')).not.toBeNull();

    rerender({ interactive: false });
    expect(portal.querySelector('[role="menu"]')).toBeNull();
    expect(container.querySelector('[data-edge-hit-target="edge-a"]')).toBeNull();

    await act(async () => Promise.resolve());
    rerender({ interactive: true });
    expect(portal.querySelector('[role="menu"]')).toBeNull();
    rerender({ interactive: false });
    rerender({ interactive: true });

    expect(portal.querySelector('[role="menu"]')).toBeNull();
    expect(consoleError).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });

  it('keeps preview edges visible but inert', () => {
    renderInteractive({ interactive: false });
    expect(container.querySelector('[data-edge-id="edge-a"]')).not.toBeNull();
    expect(container.querySelector('[data-edge-hit-target="edge-a"]')).toBeNull();
  });
});

interface RenderOptions {
  interactive?: boolean;
  initialSelection?: string[];
  initialNodeSelection?: string[];
  onCanvasPointerDown?: React.PointerEventHandler<HTMLDivElement>;
  onCanvasDoubleClick?: React.MouseEventHandler<HTMLDivElement>;
  getCanvasLocalPoint?: (clientX: number, clientY: number) => { x: number; y: number };
  onSelectionChange?: (ids: string[], graphPath: string, groupId: string) => void;
  onBreakConnections?: (ids: string[], graphPath: string, groupId: string) => boolean | Promise<boolean>;
  onEdgeDoubleClick?: (
    connectionId: string,
    position: Readonly<{ x: number; y: number }>,
    graphPath: string,
    groupId: string,
    selection: {
      before: { nodeIds: Set<string>; connectionIds: Set<string> };
      temporary: { nodeIds: Set<string>; connectionIds: Set<string> };
    },
  ) => void;
}

function installSecondProjectedEdge() {
  const fixture = makeEditorProjectionFixture({ graphPath: 'graph-a', connectionId: 'edge-a' });
  fixture.projection.connections.push({
    ...fixture.projection.connections[0],
    connectionId: 'edge-b',
  });
  useGraphDataStore.getState().replaceProjection('graph-a', fixture.projection, 2);
}


function renderDoubleClickIntegration(initialSelection: string[]) {
  updateEditorGroupSelectedConnectionIds(initialSelection, 'group-a');

  function Harness() {
    const placement = useEditorTabStore((state) => state.getPlacement('group-a'));
    const interaction = useCanvasInteraction({
      activeGroupIdRef: { current: 'group-a' },
      activeTabIdRef: { current: 'graph-a' },
      viewportRef: createRef() as React.RefObject<typeof DEFAULT_VIEWPORT>,
      setSelectedNodeIds: updateEditorGroupSelectedNodeIds,
      handlers: {
        submitConnection: vi.fn(),
        disconnectPort: vi.fn(),
        insertRerouteAtConnection: insertRerouteCommand,
        reportMutationFailure: vi.fn(),
      },
      enabled: false,
    });
    return (
      <EdgesOverlay
        graphPath="graph-a"
        groupId="group-a"
        getPinWorldPos={(pinId) => pinId.includes('local-out') ? { x: 10, y: 20 } : { x: 110, y: 80 }}
        getCanvasLocalPoint={() => ({ x: 25, y: 40 })}
        selectedNodeIds={placement.selectedNodeIds}
        selectedConnectionIds={placement.selectedConnectionIds}
        onSelectedConnectionIdsChange={(ids, _graphPath, groupId) => {
          updateEditorGroupSelectedConnectionIds(ids, groupId);
        }}
        onEdgeDoubleClick={(...args) => {
          doubleClickCompletion = interaction.insertRerouteAtConnection(...args);
        }}
      />
    );
  }

  act(() => root.render(<Harness />));
}

function dispatchDoubleClickSequence(options: {
  secondConnectionId?: string;
  secondDetail?: number;
  doubleClickConnectionId?: string;
} = {}) {
  const first = container.querySelector('[data-edge-hit-target="edge-a"]')!;
  const second = container.querySelector(
    `[data-edge-hit-target="${options.secondConnectionId ?? 'edge-a'}"]`,
  )!;
  const doubleClick = container.querySelector(
    `[data-edge-hit-target="${options.doubleClickConnectionId ?? options.secondConnectionId ?? 'edge-a'}"]`,
  )!;
  act(() => first.dispatchEvent(new MouseEvent('click', {
    bubbles: true,
    cancelable: true,
    button: 0,
    detail: 1,
    clientX: 125,
    clientY: 240,
  })));
  act(() => second.dispatchEvent(new MouseEvent('click', {
    bubbles: true,
    cancelable: true,
    button: 0,
    detail: options.secondDetail ?? 2,
    clientX: 126,
    clientY: 241,
  })));
  act(() => doubleClick.dispatchEvent(new MouseEvent('dblclick', {
    bubbles: true,
    cancelable: true,
    button: 0,
    detail: 2,
    clientX: 126,
    clientY: 241,
  })));
}

function renderInteractive(initialOptions: RenderOptions = {}, strictMode = false) {
  let options = initialOptions;
  function Harness() {
    const {
      interactive = true,
      initialSelection = [],
      initialNodeSelection = [],
      onCanvasPointerDown,
      onCanvasDoubleClick,
      getCanvasLocalPoint = vi.fn(() => ({ x: 0, y: 0 })),
      onSelectionChange = vi.fn(),
      onBreakConnections = vi.fn(),
      onEdgeDoubleClick,
    } = options;
    const [selectedConnectionIds, setSelectedConnectionIds] = useState(initialSelection);
    return (
      <div onPointerDown={onCanvasPointerDown} onDoubleClick={onCanvasDoubleClick}>
      <EdgesOverlay
        graphPath="graph-a"
        groupId="group-a"
        getPinWorldPos={(pinId) => pinId.includes('local-out') ? { x: 10, y: 20 } : { x: 110, y: 80 }}
        getCanvasLocalPoint={getCanvasLocalPoint}
        interactive={interactive}
        selectedNodeIds={initialNodeSelection}
        selectedConnectionIds={selectedConnectionIds}
        onSelectedConnectionIdsChange={(ids, graphPath, groupId) => {
          setSelectedConnectionIds(ids);
          onSelectionChange(ids, graphPath, groupId);
        }}
        onBreakConnections={onBreakConnections}
        onEdgeDoubleClick={onEdgeDoubleClick}
      />
      </div>
    );
  }

  const render = () => act(() => root.render(
    strictMode ? <React.StrictMode><Harness /></React.StrictMode> : <Harness />,
  ));
  render();
  return (next: RenderOptions) => {
    options = { ...options, ...next };
    render();
  };
}
