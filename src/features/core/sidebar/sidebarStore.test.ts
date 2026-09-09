import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PROJECT_TREE_EXPANSION_DEFAULTS, useSidebarStore } from "./sidebarStore";
import { PROJECT_TREE_CATEGORY_IDS } from "./projectTreeState";

describe("Project sidebar category expansion", () => {
  beforeEach(() => {
    useSidebarStore.setState({
      projectTreeExpandedCategories: { ...PROJECT_TREE_EXPANSION_DEFAULTS },
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("updates Project categories independently", () => {
    const store = useSidebarStore.getState();

    expect(store.projectTreeExpandedCategories).toEqual(PROJECT_TREE_EXPANSION_DEFAULTS);
    store.setProjectTreeCategoryExpanded(PROJECT_TREE_CATEGORY_IDS.functions, true);
    store.setProjectTreeCategoryExpanded(PROJECT_TREE_CATEGORY_IDS.data, false);

    expect(useSidebarStore.getState()).toMatchObject({
      projectTreeExpandedCategories: {
        ...PROJECT_TREE_EXPANSION_DEFAULTS,
        [PROJECT_TREE_CATEGORY_IDS.functions]: true,
        [PROJECT_TREE_CATEGORY_IDS.data]: false,
      },
    });
  });

  it("filters unknown persisted Project tree expansion categories", async () => {
    const persisted = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => persisted.get(key) ?? null,
      setItem: (key: string, value: string) => persisted.set(key, value),
    });
    persisted.set(
      "yssbi-project-tree-expanded-categories",
      JSON.stringify({
        [PROJECT_TREE_CATEGORY_IDS.functions]: true,
        [PROJECT_TREE_CATEGORY_IDS.data]: false,
        "project.unknown": true,
      }),
    );
    vi.resetModules();

    const fresh = await import("./sidebarStore");

    expect(fresh.useSidebarStore.getState().projectTreeExpandedCategories).toEqual({
      ...PROJECT_TREE_EXPANSION_DEFAULTS,
      [PROJECT_TREE_CATEGORY_IDS.functions]: true,
      [PROJECT_TREE_CATEGORY_IDS.data]: false,
    });
    fresh.useSidebarStore
      .getState()
      .setProjectTreeCategoryExpanded(PROJECT_TREE_CATEGORY_IDS.data, true);
    expect(JSON.parse(persisted.get("yssbi-project-tree-expanded-categories") ?? "{}")).toEqual({
      ...PROJECT_TREE_EXPANSION_DEFAULTS,
      [PROJECT_TREE_CATEGORY_IDS.functions]: true,
    });
  });
});
