import { create } from "zustand";

export type FocusedGraphSession = {
  groupId: string;
  graphPath: string;
};

interface GraphSessionState {
  /** Hydrated graph document bound to the focused editor group (at most one). */
  focusedSession: FocusedGraphSession | null;
  setFocusedSession: (groupId: string, graphPath: string) => string | null;
  clearFocusedSession: (groupId: string) => void;
  getFocusedGraphPath: () => string | null;
  getFocusedGroupId: () => string | null;
  isFocusedGraphPath: (graphPath: string) => boolean;
  remapFocusedGraphPath: (from: string, to: string) => void;
  reset: () => void;
}

export const useGraphSessionStore = create<GraphSessionState>((set, get) => ({
  focusedSession: null,

  setFocusedSession: (groupId, graphPath) => {
    const previous = get().focusedSession?.graphPath ?? null;
    set({ focusedSession: { groupId, graphPath } });
    return previous;
  },

  clearFocusedSession: (groupId) =>
    set((state) => {
      if (state.focusedSession?.groupId !== groupId) return state;
      return { focusedSession: null };
    }),

  getFocusedGraphPath: () => get().focusedSession?.graphPath ?? null,

  getFocusedGroupId: () => get().focusedSession?.groupId ?? null,

  isFocusedGraphPath: (graphPath) => get().focusedSession?.graphPath === graphPath,

  remapFocusedGraphPath: (from, to) =>
    set((state) => {
      if (state.focusedSession?.graphPath !== from) return state;
      return {
        focusedSession: { ...state.focusedSession, graphPath: to },
      };
    }),

  reset: () => set({ focusedSession: null }),
}));
