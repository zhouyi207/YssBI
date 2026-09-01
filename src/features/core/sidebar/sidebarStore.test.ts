import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  PROJECT_TREE_EXPANSION_DEFAULTS,
  SIDEBAR_SECTION_DEFAULTS,
  useSidebarStore,
} from "./sidebarStore";
import { PROJECT_TREE_CATEGORY_IDS } from "./projectTreeState";
import { mergeExpandedSections, resolveSectionExpanded } from "./sidebarSectionState";

describe("sidebarStore section expand", () => {
  beforeEach(() => {
    useSidebarStore.setState({
      expandedSections: { ...SIDEBAR_SECTION_DEFAULTS },
      projectTreeQuery: "",
      projectTreeExpandedCategories: { ...PROJECT_TREE_EXPANSION_DEFAULTS },
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("resolveSectionExpanded falls back to the Data default for unknown state", () => {
    expect(resolveSectionExpanded({}, "dataData")).toBe(true);
    expect(resolveSectionExpanded({ dataData: false }, "dataData")).toBe(false);
  });

  it("toggles the remaining Data section state", () => {
    expect(useSidebarStore.getState().isSectionExpanded("dataData")).toBe(true);

    useSidebarStore.getState().toggleSection("dataData");
    expect(useSidebarStore.getState().isSectionExpanded("dataData")).toBe(false);
  });

  it("mergeExpandedSections retains the Data default", () => {
    const merged = mergeExpandedSections({
      dataData: false,
    });
    expect(merged.dataData).toBe(false);
  });

  it("stores project tree expansion defaults separately and resets only its query", () => {
    const store = useSidebarStore.getState();

    expect(store.projectTreeExpandedCategories).toEqual(PROJECT_TREE_EXPANSION_DEFAULTS);
    store.setProjectTreeQuery("find me");
    store.setProjectTreeCategoryExpanded(PROJECT_TREE_CATEGORY_IDS.functions, true);
    store.setProjectTreeCategoriesExpanded(
      [PROJECT_TREE_CATEGORY_IDS.events, PROJECT_TREE_CATEGORY_IDS.globalVariables],
      false,
    );
    store.resetProjectTreeQuery();

    expect(useSidebarStore.getState()).toMatchObject({
      projectTreeQuery: "",
      projectTreeExpandedCategories: {
        ...PROJECT_TREE_EXPANSION_DEFAULTS,
        [PROJECT_TREE_CATEGORY_IDS.events]: false,
        [PROJECT_TREE_CATEGORY_IDS.functions]: true,
        [PROJECT_TREE_CATEGORY_IDS.globalVariables]: false,
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
        "project.unknown": true,
      }),
    );
    vi.resetModules();

    const fresh = await import("./sidebarStore");

    expect(fresh.useSidebarStore.getState().projectTreeExpandedCategories).toEqual({
      ...PROJECT_TREE_EXPANSION_DEFAULTS,
      [PROJECT_TREE_CATEGORY_IDS.functions]: true,
    });
  });

  it("normalizes legacy persisted section keys and persists only Data state", async () => {
    const persisted = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => persisted.get(key) ?? null,
      setItem: (key: string, value: string) => persisted.set(key, value),
    });
    persisted.set(
      "yssbi-sidebar-sections",
      JSON.stringify({
        graphsEvent: false,
        variablesGlobal: true,
        chartsWorksheets: false,
        dataData: false,
      }),
    );
    vi.resetModules();

    const fresh = await import("./sidebarStore");
    const store = fresh.useSidebarStore.getState();

    expect(store.expandedSections).toEqual({ dataData: false });
    store.setSectionExpanded("graphsEvent", true);
    expect(fresh.useSidebarStore.getState().expandedSections).toEqual({
      dataData: false,
    });
    store.setSectionExpanded("dataData", true);
    expect(JSON.parse(persisted.get("yssbi-sidebar-sections") ?? "{}")).toEqual({
      dataData: true,
    });
  });
});
