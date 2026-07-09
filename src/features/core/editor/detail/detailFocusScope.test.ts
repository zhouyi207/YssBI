import { beforeEach, describe, expect, it } from 'vitest';
import { useEditorStore } from '@/features/core/editor';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import {
  syncVariablesGraphScopeAfterClose,
  syncVariablesGraphScopeFromActiveTab,
} from './variablesGraphScope';

describe('variablesGraphScope', () => {
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
            tabs: [
              { id: 'g1', component: 'GraphEditor', type: 'event' },
              { id: 'g2', component: 'GraphEditor', type: 'event' },
            ],
            activeTabId: 'g2',
          },
        },
      },
      activeEditorGroupId: 'editor',
    });
    useEditorStore.getState().setVariablesGraphScope('g1');
  });

  it('moves scope to the remaining active graph tab after close', () => {
    useLayoutStore.getState().removeTab('editor', 'g1');
    syncVariablesGraphScopeAfterClose('g1');

    expect(useEditorStore.getState().variablesGraphScopePath).toBe('g2');
  });

  it('keeps scope when the last open tab is closed', () => {
    useLayoutStore.setState({
      nodes: {
        root: { id: 'root', type: 'row', parentId: null, children: ['editor'] },
        editor: {
          id: 'editor',
          type: 'component',
          parentId: 'root',
          data: {
            component: 'GraphEditor',
            tabs: [{ id: 'g1', component: 'GraphEditor', type: 'event' }],
            activeTabId: 'g1',
          },
        },
      },
      activeEditorGroupId: 'editor',
    });
    useEditorStore.getState().setVariablesGraphScope('g1');
    useLayoutStore.getState().removeTab('editor', 'g1');

    syncVariablesGraphScopeAfterClose('g1');

    expect(useEditorStore.getState().variablesGraphScopePath).toBe('g1');
  });

  it('syncs scope from active tab', () => {
    syncVariablesGraphScopeFromActiveTab();
    expect(useEditorStore.getState().variablesGraphScopePath).toBe('g2');
  });
});
