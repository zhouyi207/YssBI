import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphDataStore, useProjectIOStore } from '@/features/core/dataStore';
import { useEditorStore } from '@/features/core/editor';
import { resolveDetailTarget } from '@/features/core/editor/detail/resolveDetailTarget';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { GraphService } from '@/services/graph/graphService';
import { closeGraphTab } from './closeGraphTab';

describe('closeGraphTab', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    useLayoutStore.setState({
      rootId: 'root',
      nodes: {
        root: {
          id: 'root',
          type: 'row',
          parentId: null,
          children: ['editor'],
        },
        editor: {
          id: 'editor',
          type: 'component',
          parentId: 'root',
          data: {
            component: 'GraphEditor',
            tabs: [
              { id: 'g1', component: 'GraphEditor', type: 'event' },
              { id: 'g2', component: 'GraphEditor', type: 'event' },
            ],
            activeTabId: 'g1',
            params: { selectedNodeIds: ['node-from-g1'] },
          },
        },
      },
      activeGroupId: 'editor',
      activeEditorGroupId: 'editor',
    });
    useGraphDataStore.getState().hydrateGraphs({});
    useEditorStore.getState().clearDetailFocus();
    vi.spyOn(GraphService, 'unloadProjectGraph').mockResolvedValue();
  });

  it('loads and selects the remaining active graph after closing a tab', async () => {
    const loadGraph = vi.fn(async () => true);
    useProjectIOStore.setState({ loadGraph });

    useEditorStore.getState().setDetailFocus({ kind: 'variable', id: 'var-1' });

    const closed = await closeGraphTab('g1', 'editor', true);

    const editor = useLayoutStore.getState().nodes.editor;
    expect(closed).toBe(true);
    expect(editor.data?.activeTabId).toBe('g2');
    expect(editor.data?.params?.selectedNodeIds).toEqual([]);
    expect(loadGraph).toHaveBeenCalledWith('g2');

    const detailTarget = resolveDetailTarget({
      detailFocus: useEditorStore.getState().detailFocus,
      selectedLog: null,
    });
    expect(detailTarget).toEqual({ kind: 'variable', id: 'var-1' });
  });

  it('moves detail focus to the remaining active graph when the closed tab was focused', async () => {
    useEditorStore.getState().setDetailFocus({ kind: 'event', path: 'g1' });

    await closeGraphTab('g1', 'editor', true);

    expect(useEditorStore.getState().detailFocus).toEqual({ kind: 'event', path: 'g2' });
  });
});
