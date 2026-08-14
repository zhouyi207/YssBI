// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { useActiveEditorGroup } from './useActiveEditorGroup';

function ActiveEditorGroupProbe({
  overrideGroupId,
  resultRef,
}: {
  overrideGroupId?: string;
  resultRef: { current: ReturnType<typeof useActiveEditorGroup> | null };
}) {
  resultRef.current = useActiveEditorGroup(overrideGroupId);
  return null;
}

describe('useActiveEditorGroup', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useEditorTabStore.setState({ registry: {}, placements: {} });
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    root.unmount();
    host.remove();
  });

  it('keeps focusedEditorGroupId separate from override groupId', () => {
    useLayoutStore.setState({
      activeEditorGroupId: 'group-a',
      nodes: {
        'group-a': {
          id: 'group-a',
          type: 'component',
          parentId: null,
          data: { component: 'GraphEditor' },
        },
        'group-b': {
          id: 'group-b',
          type: 'component',
          parentId: null,
          data: { component: 'GraphEditor' },
        },
      },
    });

    const result = { current: null as ReturnType<typeof useActiveEditorGroup> | null };
    act(() => {
      root.render(<ActiveEditorGroupProbe overrideGroupId="group-b" resultRef={result} />);
    });

    expect(result.current?.groupId).toBe('group-b');
    expect(result.current?.focusedEditorGroupId).toBe('group-a');
  });

  it('projects the active group graph selection', () => {
    useLayoutStore.setState({ activeEditorGroupId: 'group-a', nodes: {} });
    useEditorTabStore.getState().ensureGroupPlacement('group-a');
    useEditorTabStore.getState().setSelectedConnectionIds('group-a', ['edge-a']);

    const result = { current: null as ReturnType<typeof useActiveEditorGroup> | null };
    act(() => {
      root.render(<ActiveEditorGroupProbe resultRef={result} />);
    });

    expect(result.current?.selection).toEqual({
      nodeIds: new Set(),
      connectionIds: new Set(['edge-a']),
    });
    expect(result.current?.selectedConnectionIds).toEqual(['edge-a']);
  });

  it('falls back to default_editor when focus is unset', () => {
    useLayoutStore.setState({
      activeEditorGroupId: null,
      nodes: {},
    });

    const result = { current: null as ReturnType<typeof useActiveEditorGroup> | null };
    act(() => {
      root.render(<ActiveEditorGroupProbe resultRef={result} />);
    });

    expect(result.current?.groupId).toBe('default_editor');
    expect(result.current?.focusedEditorGroupId).toBeNull();
  });
});
