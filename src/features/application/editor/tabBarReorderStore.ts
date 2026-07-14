import { create } from 'zustand';

export interface TabBarReorderPreview {
  targetGroupId: string;
  /** Tab reorder source group; null when inserting from sidebar graph drag. */
  sourceGroupId: string | null;
  /** Existing tab id when reordering; null for external insert preview. */
  draggedTabId: string | null;
  insertIndex: number;
  draggedIndex: number;
  gapWidth: number;
  gapLeft: number;
}

export interface ActiveTabDrag {
  tabId: string;
  sourceGroupId: string;
  title: string;
}

export interface ActiveEditorGroupDrag {
  sourceGroupId: string;
  title: string;
  tabCount: number;
}

interface TabBarReorderState {
  preview: TabBarReorderPreview | null;
  activeTabDrag: ActiveTabDrag | null;
  activeGroupDrag: ActiveEditorGroupDrag | null;
  setPreview: (preview: TabBarReorderPreview) => void;
  clearPreview: () => void;
  setActiveTabDrag: (drag: ActiveTabDrag | null) => void;
  setActiveGroupDrag: (drag: ActiveEditorGroupDrag | null) => void;
}

export const useTabBarReorderStore = create<TabBarReorderState>((set) => ({
  preview: null,
  activeTabDrag: null,
  activeGroupDrag: null,
  setPreview: (preview) => set({ preview }),
  clearPreview: () => set({ preview: null }),
  setActiveTabDrag: (activeTabDrag) => set({ activeTabDrag }),
  setActiveGroupDrag: (activeGroupDrag) => set({ activeGroupDrag }),
}));

/** Drop index for TabBar — prefers live strip preview over droppable fallback. */
export function resolveTabBarDropIndex(targetGroupId: string, fallback: number): number {
  const preview = useTabBarReorderStore.getState().preview;
  if (preview?.targetGroupId === targetGroupId) return preview.insertIndex;
  return fallback;
}

export function clearTabBarDragSession(): void {
  useTabBarReorderStore.getState().clearPreview();
  useTabBarReorderStore.getState().setActiveTabDrag(null);
  useTabBarReorderStore.getState().setActiveGroupDrag(null);
}
