import { create } from 'zustand';

interface GraphSessionState {
  activePathByGroup: Record<string, string>;
  setGroupActivePath: (groupId: string , graphPath: string) => string | null;
  clearGroupActivePath: (groupId: string) => void;
  getGroupActivePath: (groupId: string) => string | null;
  isPathActiveInAnyGroup: (graphPath: string) => boolean;
  remapActivePaths: (from: string, to: string) => void;
  reset: () => void;
}

export const useGraphSessionStore = create<GraphSessionState>((set, get) => ({
  activePathByGroup: {},

  setGroupActivePath: (groupId, graphPath) => {
    const previous = get().activePathByGroup[groupId] ?? null;
    set((state) => ({
      activePathByGroup: {
        ...state.activePathByGroup,
        [groupId]: graphPath,
      },
    }));
    return previous;
  },

  clearGroupActivePath: (groupId) =>
    set((state) => {
      if (!(groupId in state.activePathByGroup)) return state;
      const next = { ...state.activePathByGroup };
      delete next[groupId];
      return { activePathByGroup: next };
    }),

  getGroupActivePath: (groupId) => get().activePathByGroup[groupId] ?? null,

  isPathActiveInAnyGroup: (graphPath) =>
    Object.values(get().activePathByGroup).some((activePath) => activePath === graphPath),

  remapActivePaths: (from, to) =>
    set((state) => {
      const next: Record<string, string> = {};
      let changed = false;
      for (const [groupId, activePath] of Object.entries(state.activePathByGroup)) {
        if (activePath === from) {
          next[groupId] = to;
          changed = true;
        } else {
          next[groupId] = activePath;
        }
      }
      return changed ? { activePathByGroup: next } : state;
    }),

  reset: () => set({ activePathByGroup: {} }),
}));
