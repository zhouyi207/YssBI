import type { VariableListEntry } from '@/features/core/variable/variableScopeSelectors';
import {
  PROJECT_TREE_CATEGORY_IDS,
  type ProjectTreeCategoryId,
} from '@/features/core/sidebar/projectTreeState';
import type { LayoutTab } from '@/shared/types';

export interface ActiveProjectGraph {
  path: string;
  kind: 'event' | 'function';
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
    activeGraphVariables: (graphName: string) => string;
    globalVariables: string;
    noEvents: string;
    noFunctions: string;
    noWorksheets: string;
    noLocalVariables: string;
    noGlobalVariables: string;
  };
}

export type ProjectResourceBrowserCategoryRow = {
  kind: 'category';
  rowKey: string;
  categoryId: ProjectTreeCategoryId;
  level: 0;
  label: string;
  expanded: boolean;
};

export type ProjectResourceBrowserEmptyRow = {
  kind: 'empty';
  rowKey: string;
  categoryId: ProjectTreeCategoryId;
  level: 1;
  message: string;
};

export type ProjectResourceGraphRow = {
  kind: 'graph';
  rowKey: string;
  level: 1;
  id: string;
  name: string;
  graphType: 'event' | 'function';
};

export type ProjectResourceVariableRow = {
  kind: 'variable';
  rowKey: string;
  level: 1;
  id: string;
  resourcePath?: string;
  name: string;
  dataType: unknown;
  isGlobal: boolean;
};

export type ProjectResourceWorksheetRow = {
  kind: 'worksheet';
  rowKey: string;
  level: 1;
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
  activeTab: LayoutTab | null;
}): ActiveProjectGraph | null {
  const { activeTab } = input;
  if (!activeTab || (activeTab.type !== 'event' && activeTab.type !== 'function')) return null;

  const graph = activeTab.type === 'event'
    ? input.events[activeTab.id]
    : input.functions[activeTab.id];
  return graph ? { path: activeTab.id, kind: activeTab.type, name: graph.name } : null;
}

interface Category {
  id: ProjectTreeCategoryId;
  label: string;
  emptyMessage: string;
  leaves: Array<ProjectResourceGraphRow | ProjectResourceVariableRow | ProjectResourceWorksheetRow>;
}

export function buildProjectResourceBrowser(
  input: ProjectResourceBrowserInput,
): ProjectResourceBrowserProjection {
  const searching = input.query.trim().length > 0;
  const normalizedQuery = input.query.trim().toLocaleLowerCase();
  const categories = buildCategories(input).map((category) => ({
    ...category,
    leaves: searching
      ? category.leaves.filter((leaf) => leaf.name.toLocaleLowerCase().includes(normalizedQuery))
      : category.leaves,
  })).filter((category) => !searching || category.leaves.length > 0);
  const categoryIds = new Set(categories.map((category) => category.id));
  const expandedCategoryIds = searching
    ? new Set(categoryIds)
    : input.expandedCategoryIds;
  const rows: ProjectResourceBrowserRow[] = [];

  for (const category of categories) {
    const expanded = expandedCategoryIds.has(category.id);
    rows.push({
      kind: 'category',
      rowKey: `category:${category.id}`,
      categoryId: category.id,
      level: 0,
      label: category.label,
      expanded,
    });
    if (!expanded) continue;
    if (category.leaves.length > 0) {
      rows.push(...category.leaves);
    } else {
      rows.push({
        kind: 'empty',
        rowKey: `empty:${category.id}`,
        categoryId: category.id,
        level: 1,
        message: category.emptyMessage,
      });
    }
  }

  return {
    rows,
    categoryIds,
    expandedCategoryIds,
    allCategoriesExpanded: categoryIds.size > 0
      && [...categoryIds].every((categoryId) => expandedCategoryIds.has(categoryId)),
    canToggleAllCategories: !searching && categoryIds.size > 0,
  };
}

function buildCategories(input: ProjectResourceBrowserInput): Category[] {
  const categories: Category[] = [
    {
      id: PROJECT_TREE_CATEGORY_IDS.events,
      label: input.labels.events,
      emptyMessage: input.labels.noEvents,
      leaves: graphRows(input.events, 'event'),
    },
    {
      id: PROJECT_TREE_CATEGORY_IDS.functions,
      label: input.labels.functions,
      emptyMessage: input.labels.noFunctions,
      leaves: graphRows(input.functions, 'function'),
    },
    {
      id: PROJECT_TREE_CATEGORY_IDS.worksheets,
      label: input.labels.worksheets,
      emptyMessage: input.labels.noWorksheets,
      leaves: input.worksheets.map((worksheet): ProjectResourceWorksheetRow => ({
        kind: 'worksheet',
        rowKey: `worksheet:${worksheet.worksheetPath}`,
        level: 1,
        worksheetPath: worksheet.worksheetPath,
        name: worksheet.name,
      })),
    },
  ];

  if (input.activeGraph) {
    categories.push({
      id: PROJECT_TREE_CATEGORY_IDS.activeGraphVariables,
      label: input.labels.activeGraphVariables(input.activeGraph.name),
      emptyMessage: input.labels.noLocalVariables,
      leaves: variableRows(input.localVariables, false),
    });
  }

  categories.push({
    id: PROJECT_TREE_CATEGORY_IDS.globalVariables,
    label: input.labels.globalVariables,
    emptyMessage: input.labels.noGlobalVariables,
    leaves: variableRows(input.globalVariables, true),
  });
  return categories;
}

function graphRows(
  graphs: Readonly<Record<string, { name: string }>>,
  graphType: 'event' | 'function',
): ProjectResourceGraphRow[] {
  return Object.entries(graphs).map(([path, graph]) => ({
    kind: 'graph',
    rowKey: `graph:${graphType}:${path}`,
    level: 1,
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
    kind: 'variable',
    rowKey: `variable:${isGlobal ? 'global' : 'local'}:${variable.resourcePath ?? id}`,
    level: 1,
    id,
    resourcePath: variable.resourcePath,
    name: variable.name,
    dataType: variable.dataType,
    isGlobal,
  }));
}
