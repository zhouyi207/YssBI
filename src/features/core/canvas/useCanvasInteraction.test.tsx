// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CanvasInteractionHandlers } from './canvasMutationContracts';
import { useCanvasInteraction } from './useCanvasInteraction';

const graphPath = 'events/Main.yssbi-event';
const groupId = 'group-1';
type SetSelectedNodeIds = (
  updater: string[] | ((prev: string[]) => string[]),
  targetGroupId?: string,
) => void;

const mocks = vi.hoisted(() => ({
  activeTabId: 'events/Main.yssbi-event' as string | null,
  currentSelection: {
    nodeIds: new Set<string>(),
    connectionIds: new Set<string>(),
  },
  updateSelectedConnectionIds: vi.fn(),
  updateSelectedNodeIds: vi.fn(),
}));

vi.mock('@/features/core/layout/layoutTabQueries', () => ({
  getActiveLayoutTab: () => mocks.activeTabId
    ? { activeTabId: mocks.activeTabId, tab: { type: 'event' } }
    : null,
  getEditorGroupGraphSelection: () => mocks.currentSelection,
  updateEditorGroupSelectedConnectionIds: mocks.updateSelectedConnectionIds,
  updateEditorGroupSelectedNodeIds: mocks.updateSelectedNodeIds,
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('useCanvasInteraction', () => {
  let root: Root;
  let current: ReturnType<typeof useCanvasInteraction> | null;
  let setSelectedNodeIds: ReturnType<typeof vi.fn<SetSelectedNodeIds>>;
  let handlers: CanvasInteractionHandlers;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.activeTabId = graphPath;
    mocks.currentSelection = {
      nodeIds: new Set(),
      connectionIds: new Set(['connection-1']),
    };
    current = null;
    setSelectedNodeIds = vi.fn<SetSelectedNodeIds>();
    handlers = {
      submitConnection: vi.fn(),
      disconnectPort: vi.fn(),
      insertRerouteAtConnection: vi.fn().mockResolvedValue({ status: 'failed' }),
      reportMutationFailure: vi.fn(),
    };
    root = createRoot(document.createElement('div'));
    act(() => {
      root.render(<Harness setSelectedNodeIds={setSelectedNodeIds} handlers={handlers} />);
    });
  });

  afterEach(() => {
    act(() => root.unmount());
  });

  it('restores node selection through the injected setter after reroute failure', async () => {
    const before = {
      nodeIds: new Set(['node-before']),
      connectionIds: new Set<string>(),
    };
    const temporary = {
      nodeIds: new Set<string>(),
      connectionIds: new Set(['connection-1']),
    };
    mocks.currentSelection = temporary;

    await act(async () => {
      await current?.insertRerouteAtConnection(
        'connection-1',
        { x: 120, y: 80 },
        graphPath,
        groupId,
        { before, temporary },
      );
    });

    expect(setSelectedNodeIds).toHaveBeenCalledOnce();
    expect(setSelectedNodeIds).toHaveBeenCalledWith(['node-before'], groupId);
  });

  function Harness({
    setSelectedNodeIds: injectedSetSelectedNodeIds,
    handlers: injectedHandlers,
  }: {
    setSelectedNodeIds: SetSelectedNodeIds;
    handlers: CanvasInteractionHandlers;
  }) {
    current = useCanvasInteraction({
      activeGroupIdRef: { current: groupId },
      activeTabIdRef: { current: graphPath },
      viewportRef: { current: { x: 0, y: 0, scale: 1 } },
      setSelectedNodeIds: injectedSetSelectedNodeIds,
      handlers: injectedHandlers,
      enabled: false,
    });
    return null;
  }
});
