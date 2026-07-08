import { beforeEach, describe, expect, it } from 'vitest';
import { useEditorStore } from '@/features/core/editor';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import {
  applyCanvasDetailFocus,
  focusDetailOnActiveGraph,
  focusDetailOnNode,
} from './detailFocusCommands';

describe('detailFocusCommands', () => {
  beforeEach(() => {
    useLayoutStore.setState({
      rootId: 'root',
      nodes: {
        root: { id: 'root', type: 'row', parentId: null, children: ['editor'] },
        editor: {
          id: 'editor',
          type: 'component',
          parentId: 'root',
          data: {
            component: 'GraphEditor',
            tabs: [{ id: 'g1', title: 'Event', component: 'GraphEditor', type: 'event' }],
            activeTabId: 'g1',
          },
        },
      },
      activeEditorGroupId: 'editor',
    });
    useEditorStore.getState().clearDetailFocus();
  });

  it('focuses the active graph on blank click gesture', () => {
    useEditorStore.getState().setDetailFocus({ kind: 'node', id: 'n1' , graphPath: 'g1' });

    applyCanvasDetailFocus({ type: 'blank-click', groupId: 'editor' });

    expect(useEditorStore.getState().detailFocus).toEqual({ kind: 'event', path: 'g1' });
  });

  it('focuses a single node after box-select gesture', () => {
    useEditorStore.getState().setDetailFocus({ kind: 'event', path: 'g1' });

    applyCanvasDetailFocus({ type: 'box-select', groupId: 'editor', selectedIds: ['n2'] });

    expect(useEditorStore.getState().detailFocus).toEqual({ kind: 'node', id: 'n2' , graphPath: 'g1' });
  });

  it('keeps detail unchanged after multi box-select gesture', () => {
    useEditorStore.getState().setDetailFocus({ kind: 'event', path: 'g1' });

    applyCanvasDetailFocus({
      type: 'box-select',
      groupId: 'editor',
      selectedIds: ['n1', 'n2'],
    });

    expect(useEditorStore.getState().detailFocus).toEqual({ kind: 'event', path: 'g1' });
  });

  it('focuses node on node-click gesture', () => {
    applyCanvasDetailFocus({ type: 'node-click', groupId: 'editor', nodeId: 'n3' });
    expect(useEditorStore.getState().detailFocus).toEqual({ kind: 'node', id: 'n3' , graphPath: 'g1' });
  });

  it('focusDetailOnActiveGraph and focusDetailOnNode are direct helpers', () => {
    focusDetailOnActiveGraph('editor');
    expect(useEditorStore.getState().detailFocus).toEqual({ kind: 'event', path: 'g1' });

    focusDetailOnNode('n9', 'editor');
    expect(useEditorStore.getState().detailFocus).toEqual({ kind: 'node', id: 'n9' , graphPath: 'g1' });
  });
});
