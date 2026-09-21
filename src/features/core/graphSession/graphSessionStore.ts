import { create } from "zustand";

export type FocusedGraphSession = {
  groupId: string;
  graphPath: string;
};

interface GraphSessionState {
  /** Hydration bookkeeping only; active tabs and command targets come from FlexLayout. */
  focusedSession: FocusedGraphSession | null;
  setFocusedSession: (groupId: string, graphPath: string) => string | null;
  clearFocusedSession: (groupId: string) => void;
  getFocusedGroupId: () => string | null;
  isFocusedGraphPath: (graphPath: string) => boolean;
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

  getFocusedGroupId: () => get().focusedSession?.groupId ?? null,

  isFocusedGraphPath: (graphPath) => get().focusedSession?.graphPath === graphPath,

  reset: () => set({ focusedSession: null }),
}));
