// @vitest-environment happy-dom
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useHistoryStore } from '@/features/core/history';
import { HistoryService } from '@/services/nodeSystem/historyService';
import { useEditorHistoryAvailability } from './useEditorHistoryAvailability';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const activeEditor = vi.hoisted(() => ({ activeTabId: 'events/Main.yssbi-event' as string | null }));

vi.mock('@/features/core/editor/hooks/useActiveEditorGroup', () => ({
  useActiveEditorGroup: () => ({ activeTabId: activeEditor.activeTabId }),
}));
vi.mock('@/services/nodeSystem/historyService', () => ({
  HistoryService: {
    getStatus: vi.fn(async () => ({ canUndo: false, canRedo: false })),
    undo: vi.fn(),
    redo: vi.fn(),
  },
}));

describe('useEditorHistoryAvailability', () => {
  let host: HTMLDivElement;
  let root: Root;
  let current: ReturnType<typeof useEditorHistoryAvailability> | undefined;

  function Harness() {
    current = useEditorHistoryAvailability();
    return null;
  }

  beforeEach(() => {
    activeEditor.activeTabId = 'events/Main.yssbi-event';
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false }, true);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('stores only backend availability and pending state', () => {
    expect(Object.keys(useHistoryStore.getInitialState()).sort()).toEqual([
      'canRedo',
      'canUndo',
      'pending',
    ]);
  });

  it('queries backend history status when availability is first consumed', async () => {
    vi.mocked(HistoryService.getStatus).mockResolvedValueOnce({ canUndo: true, canRedo: false });

    await act(async () => {
      root.render(createElement(Harness));
      await Promise.resolve();
    });

    expect(HistoryService.getStatus).toHaveBeenCalledOnce();
    expect(current).toMatchObject({ canUndo: true, canRedo: false, pending: false });
  });

  it('uses project history status only for an active graph and masks it while pending', () => {
    useHistoryStore.setState({ canUndo: true, canRedo: true, pending: false });
    act(() => root.render(createElement(Harness)));

    expect(current).toEqual({
      activeTabId: 'events/Main.yssbi-event',
      canUndo: true,
      canRedo: true,
      pending: false,
    });

    act(() => useHistoryStore.setState({ pending: true }));
    expect(current).toMatchObject({ canUndo: false, canRedo: false, pending: true });

    activeEditor.activeTabId = null;
    act(() => root.render(createElement(Harness)));
    expect(current).toEqual({ activeTabId: null, canUndo: false, canRedo: false, pending: true });
  });
});
