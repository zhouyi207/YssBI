import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import type { ActivityPanelId } from "@/shared/types/domain/activityPanel";

const EMPTY: Readonly<Record<string, boolean>> = {};
export function useActivityPanelExpansion(panelId: ActivityPanelId) {
  const expanded = useSidebarStore((state) => state.expandedCategories[panelId] ?? EMPTY);
  const setExpanded = useSidebarStore((state) => state.setCategoryExpanded);
  return {
    expanded,
    setExpanded: (categoryId: string, value: boolean) => setExpanded(panelId, categoryId, value),
  };
}
