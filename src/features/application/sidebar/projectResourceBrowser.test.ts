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
import type { VariableListEntry } from "@/features/core/variable/variableScopeSelectors";

const labels: ProjectResourceBrowserInput["labels"] = {
  events: "Events",
  functions: "Functions",
  worksheets: "Worksheets",
  variables: "Variables",
  localVariables: "Local",
  globalVariables: "Global variables",
  noEvents: "No events",
  noFunctions: "No functions",
  noWorksheets: "No worksheets",
  noLocalVariables: "No local variables",
  noGlobalVariables: "No global variables",
};

function variable(id: string, name: string): VariableListEntry {
  return {
    id,
    name,
    resourcePath: `variables/${id}`,
    typeLabel: "String",
    dataType: { kind: "String" } as VariableListEntry["dataType"],
  };
}

function input(overrides: Partial<ProjectResourceBrowserInput> = {}): ProjectResourceBrowserInput {
  return {
    events: {},
    functions: {},
    worksheets: [],
    localVariables: {},
    globalVariables: {},
    activeGraph: null,
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
  it("keeps categories in fixed order and groups variable scopes under Variables", () => {
    const projection = buildProjectResourceBrowser(
      input({
        events: { "events/Main": { name: "Main event" } },
        functions: { "functions/Compute": { name: "Compute" } },
        worksheets: [{ worksheetPath: "worksheets/Chart", name: "Chart" }],
        activeGraph: { path: "events/Main", kind: "event", name: "Main event" },
        globalVariables: { global: variable("global", "Shared value") },
        expandedCategoryIds: new Set(Object.values(PROJECT_TREE_CATEGORY_IDS)),
      }),
    );

    expect(
      projection.rows.filter((row) => row.kind === "category").map((row) => row.categoryId),
    ).toEqual([
      PROJECT_TREE_CATEGORY_IDS.events,
      PROJECT_TREE_CATEGORY_IDS.functions,
      PROJECT_TREE_CATEGORY_IDS.worksheets,
      PROJECT_TREE_CATEGORY_IDS.variables,
      PROJECT_TREE_CATEGORY_IDS.localVariables,
      PROJECT_TREE_CATEGORY_IDS.globalVariables,
    ]);
    expect(projection.rows).toContainEqual({
      kind: "variable",
      rowKey: "variable:global:variables/global",
      level: 2,
      id: "global",
      resourcePath: "variables/global",
      name: "Shared value",
      dataType: { kind: "String" },
      isGlobal: true,
    });
  });

  it("resolves only active Event and Function tabs to project graphs", () => {
    const resources = {
      events: { "events/Main": { name: "Main event" } },
      functions: { "functions/Compute": { name: "Compute" } },
    };

    expect(
      resolveActiveProjectGraph({
        ...resources,
        activeTab: { id: "events/Main", type: "event", component: "GraphEditor" },
      }),
    ).toEqual({ path: "events/Main", kind: "event", name: "Main event" });
    expect(
      resolveActiveProjectGraph({
        ...resources,
        activeTab: { id: "functions/Compute", type: "function", component: "GraphEditor" },
      }),
    ).toEqual({ path: "functions/Compute", kind: "function", name: "Compute" });
    expect(
      resolveActiveProjectGraph({
        ...resources,
        activeTab: { id: "worksheets/Chart", type: "worksheet", component: "WorksheetEditor" },
      }),
    ).toBeNull();
  });

  it("keeps an empty Local category without an active graph", () => {
    const projection = buildProjectResourceBrowser(
      input({
        localVariables: { local: variable("local", "Local value") },
      }),
    );

    expect(projection.rows).toContainEqual({
      kind: "category",
      rowKey: `category:${PROJECT_TREE_CATEGORY_IDS.localVariables}`,
      categoryId: PROJECT_TREE_CATEGORY_IDS.localVariables,
      level: 1,
      label: "Local",
      expanded: true,
    });
    expect(projection.rows).toContainEqual({
      kind: "empty",
      rowKey: `empty:${PROJECT_TREE_CATEGORY_IDS.localVariables}`,
      categoryId: PROJECT_TREE_CATEGORY_IDS.localVariables,
      level: 2,
      message: "No local variables",
    });
    expect(projection.rows.some((row) => row.kind === "variable")).toBe(false);
  });

  it("retains an empty active-graph variables category", () => {
    const projection = buildProjectResourceBrowser(
      input({
        activeGraph: { path: "events/Main", kind: "event", name: "Main event" },
      }),
    );

    expect(projection.rows).toContainEqual({
      kind: "category",
      rowKey: `category:${PROJECT_TREE_CATEGORY_IDS.variables}`,
      categoryId: PROJECT_TREE_CATEGORY_IDS.variables,
      level: 0,
      label: "Variables",
      expanded: true,
    });
    expect(projection.rows).toContainEqual({
      kind: "category",
      rowKey: `category:${PROJECT_TREE_CATEGORY_IDS.localVariables}`,
      categoryId: PROJECT_TREE_CATEGORY_IDS.localVariables,
      level: 1,
      label: "Local",
      expanded: true,
    });
    expect(projection.rows).toContainEqual({
      kind: "empty",
      rowKey: `empty:${PROJECT_TREE_CATEGORY_IDS.localVariables}`,
      categoryId: PROJECT_TREE_CATEGORY_IDS.localVariables,
      level: 2,
      message: "No local variables",
    });
  });

  it("searches visible leaf names without mutating manual expansion", () => {
    const expandedCategoryIds = new Set([PROJECT_TREE_CATEGORY_IDS.functions]);
    const projection = buildProjectResourceBrowser(
      input({
        events: { "events/Match": { name: "Matching event" } },
        functions: { "functions/Nope": { name: "Nope" } },
        activeGraph: { path: "events/Match", kind: "event", name: "Matching event" },
        localVariables: { local: variable("local", "Matching local") },
        globalVariables: { global: variable("global", "Unrelated") },
        query: "  MATCH  ",
        expandedCategoryIds,
      }),
    );

    expect(
      projection.rows.filter((row) => row.kind === "category").map((row) => row.categoryId),
    ).toEqual([
      PROJECT_TREE_CATEGORY_IDS.events,
      PROJECT_TREE_CATEGORY_IDS.variables,
      PROJECT_TREE_CATEGORY_IDS.localVariables,
    ]);
    expect(projection.expandedCategoryIds).toEqual(
      new Set([
        PROJECT_TREE_CATEGORY_IDS.events,
        PROJECT_TREE_CATEGORY_IDS.variables,
        PROJECT_TREE_CATEGORY_IDS.localVariables,
      ]),
    );
    expect(expandedCategoryIds).toEqual(new Set([PROJECT_TREE_CATEGORY_IDS.functions]));
    expect(projection.allCategoriesExpanded).toBe(true);
    expect(projection.canToggleAllCategories).toBe(false);
  });
});
