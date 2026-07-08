import { create } from 'zustand';

export interface TabBarReorderPreview {
  targetGroupId: string;
  sourceGroupId: string;
  draggedTabId: string;
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

interface TabBarReorderState {
  preview: TabBarReorderPreview | null;
  activeTabDrag: ActiveTabDrag | null;
  setPreview: (preview: TabBarReorderPreview) => void;
  clearPreview: () => void;
  setActiveTabDrag: (drag: ActiveTabDrag | null) => void;
}

export const useTabBarReorderStore = create<TabBarReorderState>((set) => ({
  preview: null,
  activeTabDrag: null,
  setPreview: (preview) => set({ preview }),
  clearPreview: () => set({ preview: null }),
  setActiveTabDrag: (activeTabDrag) => set({ activeTabDrag }),
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
}
