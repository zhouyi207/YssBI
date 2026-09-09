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
  data: "Data",
  noEvents: "No events",
  noFunctions: "No functions",
  noCharts: "No charts",
  noData: "No data",
};

function input(overrides: Partial<ProjectResourceBrowserInput> = {}): ProjectResourceBrowserInput {
  return {
    events: {},
    functions: {},
    charts: [],
    databases: {},
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
  it("keeps data alongside graph and chart categories with its resource identity", () => {
    const database = { name: "Sales", resourcePath: "databases/Sales" };
    const projection = buildProjectResourceBrowser(
      input({
        events: { "events/Main": { name: "Main event" } },
        functions: { "functions/Compute": { name: "Compute" } },
        charts: [{ chartPath: "charts/Chart", name: "Chart" }],
        databases: { "database-id": database },
        expandedCategoryIds: new Set(Object.values(PROJECT_TREE_CATEGORY_IDS)),
      }),
    );

    expect(
      projection.rows.filter((row) => row.kind === "category").map((row) => row.categoryId),
    ).toEqual([
      PROJECT_TREE_CATEGORY_IDS.events,
      PROJECT_TREE_CATEGORY_IDS.functions,
      PROJECT_TREE_CATEGORY_IDS.charts,
      PROJECT_TREE_CATEGORY_IDS.data,
    ]);
    expect(projection.rows.find((row) => row.kind === "database")).toEqual({
      kind: "database",
      rowKey: "database:database-id",
      level: 1,
      id: "database-id",
      name: "Sales",
      resourcePath: "databases/Sales",
      data: database,
    });
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

  it("keeps every category visible and follows manual expansion", () => {
    const expandedCategoryIds = new Set([PROJECT_TREE_CATEGORY_IDS.functions]);
    const projection = buildProjectResourceBrowser(
      input({
        events: { "events/Main": { name: "Main event" } },
        functions: { "functions/Compute": { name: "Compute" } },
        databases: { "database-id": { name: "Sales" } },
        expandedCategoryIds,
      }),
    );

    expect(
      projection.rows.filter((row) => row.kind === "category").map((row) => row.categoryId),
    ).toEqual(Object.values(PROJECT_TREE_CATEGORY_IDS));
    expect(projection.rows.filter((row) => row.kind === "graph")).toMatchObject([
      { id: "functions/Compute", name: "Compute" },
    ]);
    expect(projection.rows.filter((row) => row.kind === "database")).toHaveLength(0);
    expect(expandedCategoryIds).toEqual(new Set([PROJECT_TREE_CATEGORY_IDS.functions]));
  });
});
