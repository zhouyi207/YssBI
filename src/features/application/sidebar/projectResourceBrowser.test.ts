import { describe, expect, it } from "vitest";
import {
  buildProjectResourceBrowser,
  resolveActiveProjectGraph,
  type ProjectResourceBrowserInput,
} from "./projectResourceBrowser";
import {
  PROJECT_TREE_CATEGORY_IDS,
  PROJECT_TREE_EXPANSION_DEFAULTS,
  type ProjectTreeCategoryId,
} from "@/features/core/sidebar/projectTreeState";

const labels: ProjectResourceBrowserInput["labels"] = {
  events: "Events",
  functions: "Functions",
  charts: "Charts",

  noEvents: "No events",
  noFunctions: "No functions",
  noCharts: "No charts",
};

function input(overrides: Partial<ProjectResourceBrowserInput> = {}): ProjectResourceBrowserInput {
  return {
    events: {},
    functions: {},
    charts: [],

    query: "",
    expandedCategoryIds: new Set<ProjectTreeCategoryId>(
      Object.entries(PROJECT_TREE_EXPANSION_DEFAULTS)
        .filter(([, expanded]) => expanded)
        .map(([categoryId]) => categoryId as ProjectTreeCategoryId),
    ),
    labels,
    ...overrides,
  };
}

describe("project resource browser", () => {
  it("keeps graph and chart categories in fixed order", () => {
    const projection = buildProjectResourceBrowser(
      input({
        events: { "events/Main": { name: "Main event" } },
        functions: { "functions/Compute": { name: "Compute" } },
        charts: [{ chartPath: "charts/Chart", name: "Chart" }],

        expandedCategoryIds: new Set(Object.values(PROJECT_TREE_CATEGORY_IDS)),
      }),
    );

    expect(
      projection.rows.filter((row) => row.kind === "category").map((row) => row.categoryId),
    ).toEqual([
      PROJECT_TREE_CATEGORY_IDS.events,
      PROJECT_TREE_CATEGORY_IDS.functions,
      PROJECT_TREE_CATEGORY_IDS.charts,
    ]);
  });

  it("resolves only active Event and Function editors to project graphs", () => {
    const resources = {
      events: { "events/Main": { name: "Main event" } },
      functions: { "functions/Compute": { name: "Compute" } },
    };

    expect(
      resolveActiveProjectGraph({
        ...resources,
        activeEditor: { resourceRef: "events/Main", resourceKind: "event" },
      }),
    ).toEqual({ path: "events/Main", kind: "event", name: "Main event" });
    expect(
      resolveActiveProjectGraph({
        ...resources,
        activeEditor: { resourceRef: "functions/Compute", resourceKind: "function" },
      }),
    ).toEqual({ path: "functions/Compute", kind: "function", name: "Compute" });
    expect(
      resolveActiveProjectGraph({
        ...resources,
        activeEditor: { resourceRef: "charts/Chart", resourceKind: "chart" },
      }),
    ).toBeNull();
  });

  it("searches visible leaf names without mutating manual expansion", () => {
    const expandedCategoryIds = new Set([PROJECT_TREE_CATEGORY_IDS.functions]);
    const projection = buildProjectResourceBrowser(
      input({
        events: { "events/Match": { name: "Matching event" } },
        functions: { "functions/Nope": { name: "Nope" } },

        query: "  MATCH  ",
        expandedCategoryIds,
      }),
    );

    expect(
      projection.rows.filter((row) => row.kind === "category").map((row) => row.categoryId),
    ).toEqual([PROJECT_TREE_CATEGORY_IDS.events]);
    expect(projection.expandedCategoryIds).toEqual(new Set([PROJECT_TREE_CATEGORY_IDS.events]));
    expect(expandedCategoryIds).toEqual(new Set([PROJECT_TREE_CATEGORY_IDS.functions]));
    expect(projection.allCategoriesExpanded).toBe(true);
    expect(projection.canToggleAllCategories).toBe(false);
  });
});
