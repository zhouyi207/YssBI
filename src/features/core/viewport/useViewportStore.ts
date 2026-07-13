import { create } from 'zustand';

import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';

import type { EditorViewport } from './editorViewport';
import { resolveInitialGraphViewport } from './resolveInitialGraphViewport';
import { resetLiveViewports } from './viewportSession';
import type { ViewportScope } from './viewportScope';
import { parseViewportScopeKey, viewportScopeKey } from './viewportScope';

interface ViewportStore {
  /** Committed pane viewports for this session (live gesture preview stays in viewportSession). */
  viewports: Record<string, EditorViewport>;
  setViewport: (
    scope: ViewportScope,
    updater: Partial<EditorViewport> | ((prev: EditorViewport) => EditorViewport),
  ) => void;
  clear: () => void;
}

export const useViewportStore = create<ViewportStore>((set) => ({
  viewports: {},
  setViewport: (scope, updater) =>
    set((state) => {
      const key = viewportScopeKey(scope);
      const current = state.viewports[key] ?? { ...DEFAULT_VIEWPORT };
      const next = typeof updater === 'function' ? updater(current) : { ...current, ...updater };

      if (current.x === next.x && current.y === next.y && current.scale === next.scale) {
        return state;
      }

      return {
        viewports: {
          ...state.viewports,
          [key]: next,
        },
      };
    }),
  clear: () => {
    resetLiveViewports();
    set({ viewports: {} });
  },
}));

export function remapGraphViewport(from: string, to: string): void {
  if (from === to) return;
  useViewportStore.setState((state) => {
    const viewports = { ...state.viewports };
    let changed = false;
    for (const key of Object.keys(viewports)) {
      const scope = parseViewportScopeKey(key);
      if (!scope || scope.graphPath !== from) continue;
      const nextKey = viewportScopeKey({ ...scope, graphPath: to });
      viewports[nextKey] = viewports[key];
      delete viewports[key];
      resetLiveViewports(scope);
      changed = true;
    }
    return changed ? { viewports } : state;
  });
}

export function normalizeEditorViewport(viewport?: EditorViewport | null): EditorViewport {
  if (!viewport) return { ...DEFAULT_VIEWPORT };
  return {
    x: viewport.x ?? 0,
    y: viewport.y ?? 0,
    scale: viewport.scale ?? DEFAULT_VIEWPORT.scale,
  };
}

/** Seed pane viewport on first open in a group; project memento seeds per graph path. */
export function ensureEditorViewport(scope: ViewportScope): void {
  const key = viewportScopeKey(scope);
  if (useViewportStore.getState().viewports[key]) return;
  useViewportStore.getState().setViewport(scope, resolveInitialGraphViewport(scope.graphPath));
}

/** Drop pane viewport when a tab closes in one editor group. */
export function releaseEditorViewport(scope: ViewportScope): void {
  resetLiveViewports(scope);
  useViewportStore.setState((state) => {
    const key = viewportScopeKey(scope);
    if (!(key in state.viewports)) return state;
    const viewports = { ...state.viewports };
    delete viewports[key];
    return { viewports };
  });
}

/** Drop all pane viewports for a graph when its document leaves memory. */
export function releaseGraphViewport(graphPath: string): void {
  useViewportStore.setState((state) => {
    const viewports = { ...state.viewports };
    let changed = false;
    for (const key of Object.keys(viewports)) {
      const scope = parseViewportScopeKey(key);
      if (!scope || scope.graphPath !== graphPath) continue;
      delete viewports[key];
      resetLiveViewports(scope);
      changed = true;
    }
    return changed ? { viewports } : state;
  });
}
