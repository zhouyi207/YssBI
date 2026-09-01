import type { VariableListEntry } from "@/features/core/variable/variableScopeSelectors";
import {
  PROJECT_TREE_CATEGORY_IDS,
  type ProjectTreeCategoryId,
} from "@/features/core/sidebar/projectTreeState";
import type { EditorResourceTarget } from "@/features/core/dockview";

export interface ActiveProjectGraph {
  path: string;
  kind: "event" | "function";
  name: string;
}

export interface ProjectResourceBrowserInput {
  events: Readonly<Record<string, { name: string }>>;
  functions: Readonly<Record<string, { name: string }>>;
  worksheets: readonly { worksheetPath: string; name: string }[];
  localVariables: Readonly<Record<string, VariableListEntry>>;
  globalVariables: Readonly<Record<string, VariableListEntry>>;
  activeGraph: ActiveProjectGraph | null;
  query: string;
  expandedCategoryIds: ReadonlySet<ProjectTreeCategoryId>;
  labels: {
    events: string;
    functions: string;
    worksheets: string;
    variables: string;
    localVariables: string;
    globalVariables: string;
    noEvents: string;
    noFunctions: string;
    noWorksheets: string;
    noLocalVariables: string;
    noGlobalVariables: string;
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

export type ProjectResourceVariableRow = {
  kind: "variable";
  rowKey: string;
  level: number;
  id: string;
  resourcePath?: string;
  name: string;
  dataType: unknown;
  isGlobal: boolean;
};

export type ProjectResourceWorksheetRow = {
  kind: "worksheet";
  rowKey: string;
  level: number;
  worksheetPath: string;
  name: string;
};

export type ProjectResourceBrowserRow =
  | ProjectResourceBrowserCategoryRow
  | ProjectResourceBrowserEmptyRow
  | ProjectResourceGraphRow
  | ProjectResourceVariableRow
  | ProjectResourceWorksheetRow;

export interface ProjectResourceBrowserProjection {
  rows: ProjectResourceBrowserRow[];
  categoryIds: ReadonlySet<ProjectTreeCategoryId>;
  expandedCategoryIds: ReadonlySet<ProjectTreeCategoryId>;
  allCategoriesExpanded: boolean;
  canToggleAllCategories: boolean;
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
  leaves: Array<ProjectResourceGraphRow | ProjectResourceVariableRow | ProjectResourceWorksheetRow>;
  children?: Category[];
}

export function buildProjectResourceBrowser(
  input: ProjectResourceBrowserInput,
): ProjectResourceBrowserProjection {
  const searching = input.query.trim().length > 0;
  const normalizedQuery = input.query.trim().toLocaleLowerCase();
  const categories = buildCategories(input)
    .map((category) => filterCategory(category, searching, normalizedQuery))
    .filter((category): category is Category => category !== null);
  const categoryIds = new Set<ProjectTreeCategoryId>();
  for (const category of categories) collectCategoryIds(category, categoryIds);
  const expandedCategoryIds = searching ? new Set(categoryIds) : input.expandedCategoryIds;
  const rows: ProjectResourceBrowserRow[] = [];

  for (const category of categories) {
    appendCategoryRows(category, 0, expandedCategoryIds, rows);
  }

  return {
    rows,
    categoryIds,
    expandedCategoryIds,
    allCategoriesExpanded:
      categoryIds.size > 0 &&
      [...categoryIds].every((categoryId) => expandedCategoryIds.has(categoryId)),
    canToggleAllCategories: !searching && categoryIds.size > 0,
  };
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
      id: PROJECT_TREE_CATEGORY_IDS.worksheets,
      label: input.labels.worksheets,
      emptyMessage: input.labels.noWorksheets,
      leaves: input.worksheets.map((worksheet): ProjectResourceWorksheetRow => ({
        kind: "worksheet",
        rowKey: `worksheet:${worksheet.worksheetPath}`,
        level: 0,
        worksheetPath: worksheet.worksheetPath,
        name: worksheet.name,
      })),
    },
  ];

  categories.push({
    id: PROJECT_TREE_CATEGORY_IDS.variables,
    label: input.labels.variables,
    leaves: [],
    children: [
      {
        id: PROJECT_TREE_CATEGORY_IDS.localVariables,
        label: input.labels.localVariables,
        emptyMessage: input.labels.noLocalVariables,
        leaves: input.activeGraph ? variableRows(input.localVariables, false) : [],
      },
      {
        id: PROJECT_TREE_CATEGORY_IDS.globalVariables,
        label: input.labels.globalVariables,
        emptyMessage: input.labels.noGlobalVariables,
        leaves: variableRows(input.globalVariables, true),
      },
    ],
  });
  return categories;
}

function filterCategory(
  category: Category,
  searching: boolean,
  normalizedQuery: string,
): Category | null {
  const leaves = searching
    ? category.leaves.filter((leaf) => leaf.name.toLocaleLowerCase().includes(normalizedQuery))
    : category.leaves;
  const children = category.children
    ?.map((child) => filterCategory(child, searching, normalizedQuery))
    .filter((child): child is Category => child !== null);
  const hasVisibleContent = leaves.length > 0 || (children?.length ?? 0) > 0;
  if (searching && !hasVisibleContent) return null;
  return { ...category, leaves, children };
}

function collectCategoryIds(category: Category, categoryIds: Set<ProjectTreeCategoryId>): void {
  categoryIds.add(category.id);
  for (const child of category.children ?? []) collectCategoryIds(child, categoryIds);
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

function variableRows(
  variables: Readonly<Record<string, VariableListEntry>>,
  isGlobal: boolean,
): ProjectResourceVariableRow[] {
  return Object.entries(variables).map(([id, variable]) => ({
    kind: "variable",
    rowKey: `variable:${isGlobal ? "global" : "local"}:${variable.resourcePath ?? id}`,
    level: 0,
    id,
    resourcePath: variable.resourcePath,
    name: variable.name,
    dataType: variable.dataType,
    isGlobal,
  }));
}
