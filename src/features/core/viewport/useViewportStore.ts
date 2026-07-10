import { create } from 'zustand';

import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';

import type { EditorViewport } from './editorViewport';

import { resolveInitialGraphViewport } from './resolveInitialGraphViewport';

import { resetLiveViewports } from './viewportSession';



interface ViewportStore {

  /** Committed viewports for this session (live gesture preview stays in viewportSession). */

  viewports: Record<string, EditorViewport>;

  setViewport: (

    graphPath: string,

    updater: Partial<EditorViewport> | ((prev: EditorViewport) => EditorViewport),

  ) => void;

  clear: () => void;

}



export const useViewportStore = create<ViewportStore>((set) => ({

  viewports: {},

  setViewport: (graphPath, updater) =>

    set((state) => {

      const current = state.viewports[graphPath] ?? { ...DEFAULT_VIEWPORT };

      const next = typeof updater === 'function' ? updater(current) : { ...current, ...updater };



      if (current.x === next.x && current.y === next.y && current.scale === next.scale) {

        return state;

      }



      return {

        viewports: {

          ...state.viewports,

          [graphPath]: next,

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

  resetLiveViewports(from);

  useViewportStore.setState((state) => {

    const viewport = state.viewports[from];

    if (!viewport) return state;

    const viewports = { ...state.viewports };

    delete viewports[from];

    viewports[to] = viewport;

    return { viewports };

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



/** Seed viewport on first open; session edits stay in memory until persisted to editor view state. */

export function ensureGraphViewport(graphPath: string): void {

  if (useViewportStore.getState().viewports[graphPath]) return;

  useViewportStore.getState().setViewport(graphPath, resolveInitialGraphViewport(graphPath));

}



/** Drop committed + live viewport when a graph document is unloaded from memory. */

export function releaseGraphViewport(graphPath: string): void {

  resetLiveViewports(graphPath);

  useViewportStore.setState((state) => {

    if (!(graphPath in state.viewports)) return state;

    const viewports = { ...state.viewports };

    delete viewports[graphPath];

    return { viewports };

  });

}


