import { create } from "zustand";
import type { ActivityPanelId, ActivityPanelSnapshot } from "@/shared/types/domain/activityPanel";

import type { ErrorReference } from "@/shared/types/domain/diagnostics";

export interface ActivityPanelBinding {
  readonly panelId: ActivityPanelId;
  readonly projectInstanceId: string | null;
  readonly locale: string;
  readonly epoch: number;
}
export interface ActivityPanelState {
  readonly binding: ActivityPanelBinding;
  readonly snapshot: ActivityPanelSnapshot | null;
  readonly loading: boolean;
  readonly error: ErrorReference | null;
}
export interface ActivityPanelPublication {
  readonly binding: ActivityPanelBinding;
  readonly snapshot: ActivityPanelSnapshot;
}

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
  panels: Partial<Record<ActivityPanelId, ActivityPanelState>>;
  bindPanel(binding: ActivityPanelBinding): ActivityPanelBinding;
  startPanelRequest(binding: ActivityPanelBinding): void;
  publishPanels(updates: readonly ActivityPanelPublication[]): void;
  failPanelRequest(binding: ActivityPanelBinding, error: ErrorReference): void;
  clearProjectPanels(): void;
  setCategoryExpanded(panelId: ActivityPanelId, categoryId: string, expanded: boolean): void;
}>((set, get) => ({
  expandedCategories: loadExpansion(),
  panels: {},
  bindPanel: (binding) => {
    const current = get().panels[binding.panelId];
    if (
      current &&
      current.binding.projectInstanceId === binding.projectInstanceId &&
      current.binding.locale === binding.locale &&
      current.binding.epoch === binding.epoch
    )
      return current.binding;
    set((state) => ({
      panels: {
        ...state.panels,
        [binding.panelId]: { binding, snapshot: null, loading: false, error: null },
      },
    }));
    return binding;
  },
  startPanelRequest: (binding) =>
    set((state) => {
      const current = state.panels[binding.panelId];
      if (current?.binding !== binding || current.loading) return state;
      return { panels: { ...state.panels, [binding.panelId]: { ...current, loading: true } } };
    }),
  publishPanels: (updates) =>
    set((state) => {
      let panels = state.panels;
      for (const { binding, snapshot } of updates) {
        const current = panels[binding.panelId];
        if (
          current?.binding !== binding ||
          (current.snapshot === snapshot && !current.loading && !current.error)
        )
          continue;
        if (panels === state.panels) panels = { ...panels };
        panels[binding.panelId] = { binding, snapshot, loading: false, error: null };
      }
      return panels === state.panels ? state : { panels };
    }),
  failPanelRequest: (binding, error) =>
    set((state) => {
      const current = state.panels[binding.panelId];
      if (current?.binding !== binding) return state;
      return {
        panels: { ...state.panels, [binding.panelId]: { ...current, loading: false, error } },
      };
    }),
  clearProjectPanels: () =>
    set((state) => {
      const { project: _project, nodes: _nodes, ...panels } = state.panels;
      return { panels };
    }),

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
