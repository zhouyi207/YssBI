import {
  PROJECT_TREE_CATEGORY_IDS,
  type ProjectTreeCategoryId,
} from "@/features/core/sidebar/projectTreeState";
import type { EditorResourceTarget } from "@/modules/workbench/public";

export interface ActiveProjectGraph {
  path: string;
  kind: "event" | "function";
  name: string;
}

export interface ProjectResourceBrowserInput {
  events: Readonly<Record<string, { name: string }>>;
  functions: Readonly<Record<string, { name: string }>>;
  charts: readonly { chartPath: string; name: string }[];
  databases: Readonly<Record<string, { name: string; resourcePath?: string }>>;
  expandedCategoryIds: ReadonlySet<ProjectTreeCategoryId>;
  labels: {
    events: string;
    functions: string;
    charts: string;
    data: string;
    noEvents: string;
    noFunctions: string;
    noCharts: string;
    noData: string;
  };
}

export type ProjectResourceBrowserCategoryRow = {
  kind: "category";
  rowKey: string;
  categoryId: ProjectTreeCategoryId;
  level: number;
  label: string;
  expanded: boolean;
};

export type ProjectResourceBrowserEmptyRow = {
  kind: "empty";
  rowKey: string;
  categoryId: ProjectTreeCategoryId;
  level: number;
  message: string;
};

export type ProjectResourceGraphRow = {
  kind: "graph";
  rowKey: string;
  level: number;
  id: string;
  name: string;
  graphType: "event" | "function";
};

export type ProjectResourceChartRow = {
  kind: "chart";
  rowKey: string;
  level: number;
  chartPath: string;
  name: string;
};

export type ProjectResourceDatabaseRow = {
  kind: "database";
  rowKey: string;
  level: number;
  id: string;
  name: string;
  resourcePath?: string;
  data: unknown;
};

export type ProjectResourceBrowserRow =
  | ProjectResourceBrowserCategoryRow
  | ProjectResourceBrowserEmptyRow
  | ProjectResourceGraphRow
  | ProjectResourceChartRow
  | ProjectResourceDatabaseRow;

export interface ProjectResourceBrowserProjection {
  rows: ProjectResourceBrowserRow[];
}

export function resolveActiveProjectGraph(input: {
  events: Readonly<Record<string, { name: string }>>;
  functions: Readonly<Record<string, { name: string }>>;
  activeEditor: EditorResourceTarget | null;
}): ActiveProjectGraph | null {
  const { activeEditor } = input;
  if (
    !activeEditor ||
    (activeEditor.resourceKind !== "event" && activeEditor.resourceKind !== "function")
  )
    return null;

  const graph =
    activeEditor.resourceKind === "event"
      ? input.events[activeEditor.resourceRef]
      : input.functions[activeEditor.resourceRef];
  return graph
    ? {
        path: activeEditor.resourceRef,
        kind: activeEditor.resourceKind,
        name: graph.name,
      }
    : null;
}

interface Category {
  id: ProjectTreeCategoryId;
  label: string;
  emptyMessage?: string;
  leaves: Array<ProjectResourceGraphRow | ProjectResourceChartRow | ProjectResourceDatabaseRow>;
  children?: Category[];
}

export function buildProjectResourceBrowser(
  input: ProjectResourceBrowserInput,
): ProjectResourceBrowserProjection {
  const rows: ProjectResourceBrowserRow[] = [];

  for (const category of buildCategories(input)) {
    appendCategoryRows(category, 0, input.expandedCategoryIds, rows);
  }

  return { rows };
}

function buildCategories(input: ProjectResourceBrowserInput): Category[] {
  const categories: Category[] = [
    {
      id: PROJECT_TREE_CATEGORY_IDS.events,
      label: input.labels.events,
      emptyMessage: input.labels.noEvents,
      leaves: graphRows(input.events, "event"),
    },
    {
      id: PROJECT_TREE_CATEGORY_IDS.functions,
      label: input.labels.functions,
      emptyMessage: input.labels.noFunctions,
      leaves: graphRows(input.functions, "function"),
    },
    {
      id: PROJECT_TREE_CATEGORY_IDS.charts,
      label: input.labels.charts,
      emptyMessage: input.labels.noCharts,
      leaves: input.charts.map((chart): ProjectResourceChartRow => ({
        kind: "chart",
        rowKey: `chart:${chart.chartPath}`,
        level: 0,
        chartPath: chart.chartPath,
        name: chart.name,
      })),
    },
    {
      id: PROJECT_TREE_CATEGORY_IDS.data,
      label: input.labels.data,
      emptyMessage: input.labels.noData,
      leaves: Object.entries(input.databases).map(([id, data]): ProjectResourceDatabaseRow => ({
        kind: "database",
        rowKey: `database:${id}`,
        level: 0,
        id,
        name: data.name,
        resourcePath: data.resourcePath,
        data,
      })),
    },
  ];

  return categories;
}

function appendCategoryRows(
  category: Category,
  level: number,
  expandedCategoryIds: ReadonlySet<ProjectTreeCategoryId>,
  rows: ProjectResourceBrowserRow[],
): void {
  const expanded = expandedCategoryIds.has(category.id);
  rows.push({
    kind: "category",
    rowKey: `category:${category.id}`,
    categoryId: category.id,
    level,
    label: category.label,
    expanded,
  });
  if (!expanded) return;

  for (const child of category.children ?? []) {
    appendCategoryRows(child, level + 1, expandedCategoryIds, rows);
  }
  if (category.leaves.length > 0) {
    rows.push(...category.leaves.map((leaf) => ({ ...leaf, level: level + 1 })));
  } else if (!category.children?.length && category.emptyMessage) {
    rows.push({
      kind: "empty",
      rowKey: `empty:${category.id}`,
      categoryId: category.id,
      level: level + 1,
      message: category.emptyMessage,
    });
  }
}

function graphRows(
  graphs: Readonly<Record<string, { name: string }>>,
  graphType: "event" | "function",
): ProjectResourceGraphRow[] {
  return Object.entries(graphs).map(([path, graph]) => ({
    kind: "graph",
    rowKey: `graph:${graphType}:${path}`,
    level: 0,
    id: path,
    name: graph.name,
    graphType,
  }));
}
