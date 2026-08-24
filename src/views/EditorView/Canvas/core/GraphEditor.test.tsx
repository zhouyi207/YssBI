// @vitest-environment happy-dom

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GroupContext } from '@/features/core/editor';
import { GraphEditor } from './GraphEditor';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const editorState = vi.hoisted(() => ({
  activeTabId: null as string | null,
  tabs: [] as Array<{ id: string; type: 'event' | 'function' }>,
  projectionReady: false,
  documentLoaded: false,
  documentStale: false,
  documentConflict: false,
  loadStatus: undefined as 'loading' | 'ready' | 'error' | undefined,
}));

vi.mock('@/features/application/editor', () => ({
  useIsActiveEditorGroup: () => true,
}));

vi.mock('@/features/core/editor', async () => {
  const { createContext } = await import('react');
  return {
    GroupContext: createContext<string | null>(null),
    useEditorGroupWorkspace: () => ({
      activeTabId: editorState.activeTabId,
      tabs: editorState.tabs,
    }),
  };
});

vi.mock('@/features/core/dataStore', () => ({
  useGraphDataStore: (selector: (state: { hasGraph: (path: string) => boolean }) => unknown) =>
    selector({ hasGraph: () => editorState.projectionReady }),
  useProjectIOStore: (selector: (state: { graphLoadStatus: Record<string, string> }) => unknown) =>
    selector({
      graphLoadStatus: editorState.activeTabId && editorState.loadStatus
        ? { [editorState.activeTabId]: editorState.loadStatus }
        : {},
    }),
}));

vi.mock('@/features/core/resource', () => ({
  resourceKey: ({ id, kind }: { id: string; kind: string }) => `${kind}:${id}`,
  useDocumentStateStore: (selector: (state: {
    documents: Record<string, { loaded: boolean; stale: boolean; conflict: boolean }>;
  }) => unknown) =>
    selector({
      documents: editorState.activeTabId && editorState.documentLoaded
        ? { [`event:${editorState.activeTabId}`]: {
            loaded: true,
            stale: editorState.documentStale,
            conflict: editorState.documentConflict,
          } }
        : {},
    }),
}));

vi.mock('./Canvas', () => ({ default: () => <div data-canvas /> }));
vi.mock('../overlays/WatermarkView', () => ({ WatermarkView: () => <div data-watermark /> }));
vi.mock('./CanvasDropZone', () => ({
  CanvasDropZone: ({ children, groupId }: { children: ReactNode; groupId: string }) => (
    <div data-drop-zone data-group-id={groupId}>{children}</div>
  ),
}));

function scopedGraphEditor(key?: string) {
  return (
    <GroupContext.Provider value="group-a">
      <GraphEditor key={key} />
    </GroupContext.Provider>
  );
}

describe('GraphEditor readiness and sizing', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    editorState.activeTabId = null;
    editorState.tabs = [];
    editorState.projectionReady = false;
    editorState.documentLoaded = false;
    editorState.documentStale = false;
    editorState.documentConflict = false;
    editorState.loadStatus = undefined;
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('allows both shell layers to shrink within flex layouts', () => {
    act(() => root.render(scopedGraphEditor()));

    const outer = host.firstElementChild as HTMLElement;
    const inner = outer.firstElementChild as HTMLElement;

    expect(outer.classList).toContain('min-h-0');
    expect(outer.classList).toContain('min-w-0');
    expect(inner.classList).toContain('min-h-0');
    expect(inner.classList).toContain('min-w-0');
    expect(host.querySelector('[data-drop-zone]')?.getAttribute('data-group-id')).toBe('group-a');
  });

  it('keeps a stable loading shell until document and projection are both ready', () => {
    editorState.activeTabId = 'events/Main.yssbi-event';
    editorState.tabs = [{ id: editorState.activeTabId, type: 'event' }];
    editorState.loadStatus = 'loading';

    act(() => root.render(scopedGraphEditor()));

    expect(host.querySelector('[data-graph-loading]')).not.toBeNull();
    expect(host.querySelector('[data-canvas]')).toBeNull();

    editorState.projectionReady = true;
    act(() => root.render(scopedGraphEditor('projection-ready')));
    expect(host.querySelector('[data-canvas]')).toBeNull();

    editorState.documentLoaded = true;
    editorState.loadStatus = 'ready';
    act(() => root.render(scopedGraphEditor('document-ready')));
    expect(host.querySelector('[data-graph-loading]')).toBeNull();
    expect(host.querySelector('[data-canvas]')).not.toBeNull();
  });

  it('exits the loading state when graph loading fails', () => {
    editorState.activeTabId = 'events/Failed.yssbi-event';
    editorState.tabs = [{ id: editorState.activeTabId, type: 'event' }];
    editorState.loadStatus = 'error';

    act(() => root.render(scopedGraphEditor()));

    expect(host.querySelector('[data-graph-loading]')).toBeNull();
    expect(host.querySelector('[data-graph-load-error]')).not.toBeNull();
    expect(host.querySelector('[data-canvas]')).toBeNull();
  });

  it('does not render a stale projection over a failed refresh state', () => {
    editorState.activeTabId = 'events/Stale.yssbi-event';
    editorState.tabs = [{ id: editorState.activeTabId, type: 'event' }];
    editorState.projectionReady = true;
    editorState.documentLoaded = true;
    editorState.documentStale = true;
    editorState.loadStatus = 'error';

    act(() => root.render(scopedGraphEditor()));

    expect(host.querySelector('[data-graph-load-error]')).not.toBeNull();
    expect(host.querySelector('[data-canvas]')).toBeNull();
  });
});
