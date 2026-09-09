import { create } from "zustand";
import type { ActivityPanelId } from "@/shared/types/domain/activityPanel";

const STORAGE_KEY = "yssbi-activity-panel-expansion";
type Expansion = Partial<Record<ActivityPanelId, Record<string, boolean>>>;

function loadExpansion(): Expansion {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    if (!value || typeof value !== "object" || Array.isArray(value)) return {};
    return Object.fromEntries(
      Object.entries(value)
        .filter(
          ([panelId, categories]) =>
            ["project", "nodes", "commands", "plugins"].includes(panelId) &&
            categories &&
            typeof categories === "object" &&
            !Array.isArray(categories),
        )
        .map(([panelId, categories]) => [
          panelId,
          Object.fromEntries(
            Object.entries(categories).filter(([, expanded]) => typeof expanded === "boolean"),
          ),
        ]),
    );
  } catch {
    return {};
  }
}

export const useSidebarStore = create<{
  expandedCategories: Expansion;
  setCategoryExpanded(panelId: ActivityPanelId, categoryId: string, expanded: boolean): void;
}>((set) => ({
  expandedCategories: loadExpansion(),
  setCategoryExpanded: (panelId, categoryId, expanded) =>
    set((state) => {
      const expandedCategories = {
        ...state.expandedCategories,
        [panelId]: { ...state.expandedCategories[panelId], [categoryId]: expanded },
      };
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(expandedCategories));
      } catch {
        /* UI preferences can remain session-local. */
      }
      return { expandedCategories };
    }),
}));
