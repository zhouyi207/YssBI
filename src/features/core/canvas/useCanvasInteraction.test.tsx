// @vitest-environment happy-dom
import { act, createRef } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { getCanvasInteraction, useGraphInteractionStore } from '@/features/core/graphInteraction/graphInteractionStore';
import { updateEditorGroupSelectedNodeIds } from '@/features/core/layout/layoutTabQueries';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';

import {
  resolvePinPointerAction,
  useCanvasInteraction,
  type CanvasInteractionHandlers,
} from './useCanvasInteraction';

const movable = { current: 1, maximum: 1, ordered: false, canAppend: false, canReplace: true, canMove: true };

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('resolvePinPointerAction', () => {
  it('routes Alt before Ctrl/Meta to one port disconnect', () => {
    expect(resolvePinPointerAction({ button: 0, altKey: true, ctrlKey: true, metaKey: true }, movable)).toBe('disconnect');
  });

  it.each([
    [{ button: 0, ctrlKey: true }, 'move'],
    [{ button: 0, metaKey: true }, 'move'],
  ] as const)('routes occupied movable modifier drag to move', (event, action) => {
    expect(resolvePinPointerAction(event, movable)).toBe(action);
  });

  it('requires occupied canMove for move and append-or-replace for drawing', () => {
    expect(resolvePinPointerAction({ button: 0, ctrlKey: true }, { ...movable, current: 0 })).toBe('none');
    expect(resolvePinPointerAction({ button: 0 }, { ...movable, canMove: false })).toBe('draw');
    expect(resolvePinPointerAction({ button: 0 }, { ...movable, canAppend: false, canReplace: false })).toBe('none');
  });
});

describe('reroute insertion', () => {
  const graphPath = 'events/main';
  const groupId = 'group-a';
  let host: HTMLDivElement;
  let root: Root;
  let interaction: ReturnType<typeof useCanvasInteraction>;
  let handlers: CanvasInteractionHandlers;

  function Harness() {
    interaction = useCanvasInteraction({
      activeGroupIdRef: { current: groupId },
      activeTabIdRef: { current: graphPath },
      viewportRef: createRef() as React.RefObject<typeof DEFAULT_VIEWPORT>,
      setSelectedNodeIds: updateEditorGroupSelectedNodeIds,
      handlers,
      enabled: false,
    });
    return null;
  }

  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {
      submitConnection: vi.fn(),
      disconnectPort: vi.fn(),
      insertRerouteAtConnection: vi.fn(),
      reportMutationFailure: vi.fn(),
    };
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useGraphInteractionStore.setState({
      interactions: {
        [graphPath]: {
          type: 'idle',
        },
      },
      positionOverrides: {},
    });
    useEditorTabStore.getState().initGroupPlacement(groupId, [
      { id: graphPath, component: 'GraphEditor', type: 'event' },
    ], graphPath);
    useEditorTabStore.getState().setSelectedConnectionIds(groupId, ['edge-a']);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  const doubleClickSelection = {
    before: { nodeIds: new Set<string>(), connectionIds: new Set(['edge-b']) },
    temporary: { nodeIds: new Set<string>(), connectionIds: new Set(['edge-a']) },
  };

  it('sends one insertion and clears only the replaced edge after an applied result', async () => {
    vi.mocked(handlers.insertRerouteAtConnection).mockResolvedValueOnce({ status: 'applied' });

    await act(async () => interaction.insertRerouteAtConnection(
      'edge-a',
      { x: 25, y: 40 },
      graphPath,
      groupId,
      doubleClickSelection,
    ));

    expect(handlers.insertRerouteAtConnection).toHaveBeenCalledTimes(1);
    expect(handlers.insertRerouteAtConnection).toHaveBeenCalledWith({ graphPath, connectionId: 'edge-a', position: { x: 25, y: 40 } });
    expect(useEditorTabStore.getState().getPlacement(groupId).selectedConnectionIds).toEqual([]);
  });

  it.each([
    ['local invalid', false],
    ['noop', { status: 'noop', result: {} }],
    ['stale', { status: 'stale' }],
    ['rejected', { status: 'rejected', code: 'graph_connection_not_found' }],
    ['conflict', { status: 'conflict' }],
  ] as const)('restores the pre-double-click selection for %s', async (_label, _outcome) => {
    vi.mocked(handlers.insertRerouteAtConnection).mockResolvedValueOnce({ status: 'failed' });

    await act(async () => interaction.insertRerouteAtConnection(
      'edge-a',
      { x: 25, y: 40 },
      graphPath,
      groupId,
      doubleClickSelection,
    ));

    expect(useEditorTabStore.getState().getPlacement(groupId).selectedConnectionIds).toEqual(['edge-b']);
  });

  it('restores the pre-double-click selection when the command throws', async () => {
    vi.mocked(handlers.insertRerouteAtConnection).mockRejectedValueOnce(new Error('transport failed'));

    await act(async () => interaction.insertRerouteAtConnection(
      'edge-a',
      { x: 25, y: 40 },
      graphPath,
      groupId,
      doubleClickSelection,
    ));

    expect(useEditorTabStore.getState().getPlacement(groupId).selectedConnectionIds).toEqual(['edge-b']);
  });

  it('does not overwrite a user selection changed while insertion is pending', async () => {
    vi.mocked(handlers.insertRerouteAtConnection).mockImplementationOnce(async () => {
      useEditorTabStore.getState().setSelectedConnectionIds(groupId, ['edge-user']);
      return { status: 'failed' };
    });

    await act(async () => interaction.insertRerouteAtConnection(
      'edge-a',
      { x: 25, y: 40 },
      graphPath,
      groupId,
      doubleClickSelection,
    ));

    expect(useEditorTabStore.getState().getPlacement(groupId).selectedConnectionIds).toEqual(['edge-user']);
  });
});

describe('node pointer selection', () => {
  const graphPath = 'events/main';
  const groupId = 'group-a';
  let host: HTMLDivElement;
  let root: Root;
  let interaction: ReturnType<typeof useCanvasInteraction>;

  function Harness() {
    interaction = useCanvasInteraction({
      activeGroupIdRef: { current: groupId },
      activeTabIdRef: { current: graphPath },
      viewportRef: createRef() as React.RefObject<typeof DEFAULT_VIEWPORT>,
      setSelectedNodeIds: updateEditorGroupSelectedNodeIds,
      handlers: {
        submitConnection: vi.fn(),
        disconnectPort: vi.fn(),
        insertRerouteAtConnection: vi.fn(),
        reportMutationFailure: vi.fn(),
      },
      enabled: false,
    });
    return null;
  }

  beforeEach(() => {
    vi.clearAllMocks();
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useEditorTabStore.getState().initGroupPlacement(groupId, [
      { id: graphPath, component: 'GraphEditor', type: 'event' },
    ], graphPath);
    useGraphInteractionStore.setState({
      interactions: {
        [graphPath]: {
          type: 'panning',
          session: {
            groupId,
            startX: 0,
            startY: 0,
            lastX: 0,
            lastY: 0,
            moved: false,
          },
        },
      },
      positionOverrides: {},
    });
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  function pointerDown(nodeId: string, modifiers: Partial<React.PointerEvent> = {}) {
    act(() => interaction.onNodePointerDown(nodeId, {
      button: 0,
      clientX: 10,
      clientY: 20,
      shiftKey: false,
      ctrlKey: false,
      metaKey: false,
      stopPropagation: vi.fn(),
      ...modifiers,
    } as React.PointerEvent, groupId));
  }

  it.each([
    ['Shift', { shiftKey: true }],
    ['Ctrl', { ctrlKey: true }],
    ['Meta', { metaKey: true }],
  ] as const)('toggles with %s and builds drag from the final selection', (_label, modifiers) => {
    act(() => useEditorTabStore.getState().setSelectedNodeIds(groupId, ['node-a', 'node-b']));

    pointerDown('node-a', modifiers);

    expect(useEditorTabStore.getState().getPlacement(groupId).selectedNodeIds).toEqual(['node-b']);
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId)).toMatchObject({
      type: 'draggingNodes',
      session: { nodeIds: ['node-b'] },
    });
  });

  it('replaces an existing multi-selection without modifiers and clears edge selection', () => {
    act(() => useEditorTabStore.getState().setSelectedNodeIds(groupId, ['node-a', 'node-b']));
    pointerDown('node-a');
    expect(useEditorTabStore.getState().getPlacement(groupId).selectedNodeIds).toEqual(['node-a']);

    act(() => useEditorTabStore.getState().setSelectedConnectionIds(groupId, ['edge-a']));
    pointerDown('node-b');
    expect(useEditorTabStore.getState().getPlacement(groupId)).toMatchObject({
      selectedNodeIds: ['node-b'],
      selectedConnectionIds: [],
    });
  });
});
