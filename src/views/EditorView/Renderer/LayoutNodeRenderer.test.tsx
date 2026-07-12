// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { LayoutNodeRenderer } from './LayoutNodeRenderer';

vi.mock('./viewRegistry', () => ({
  viewRegistry: {
    get: () => () => <div data-full-editor-content="true" />,
  },
}));

vi.mock('./Sash', () => ({
  Sash: () => null,
}));

vi.mock('../Layout/TabBar', () => ({
  TabBar: () => null,
}));

vi.mock('@/features/core/editor/hooks/useEditorGroupTabStrip', () => ({
  useEditorGroupTabStrip: () => ({ tabs: [], activeTabId: undefined }),
}));

vi.mock('@dnd-kit/core', () => ({
  useDroppable: () => ({ setNodeRef: vi.fn() }),
}));

describe('LayoutNodeRenderer maximize-hidden groups', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    useLayoutStore.setState({
      rootId: 'editor-area',
      activeEditorGroupId: 'visible-group',
      isDragging: false,
      nodes: {
        'editor-area': {
          id: 'editor-area',
          type: 'row',
          parentId: null,
          children: ['hidden-group'],
        },
        'hidden-group': {
          id: 'hidden-group',
          type: 'component',
          parentId: 'editor-area',
          data: {
            component: 'GraphEditor',
            tabs: [],
            groupMaximizedHidden: true,
          },
        },
      },
    });
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    host.remove();
  });

  it('keeps only a lightweight placeholder instead of mounting editor content', async () => {
    await act(async () => {
      root.render(<LayoutNodeRenderer nodeId="editor-area" />);
    });

    expect(host.querySelector('[data-full-editor-content]')).toBeNull();
    expect(host.querySelector('[data-editor-group-maximized-placeholder="hidden-group"]')).not.toBeNull();
  });
});
