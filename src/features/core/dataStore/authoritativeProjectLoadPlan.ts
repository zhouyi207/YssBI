import type { ProjectData, Variable } from '@/shared/types';
import type { DatabaseRecord } from '@/shared/types/dto/database';
import { normalizeDatabases } from '@/shared/types/dto/database';
import type { ProjectGraphIndexRow, ProjectIndexRow } from '@/services/project/projectService';
import {
  applyVariableCatalogFromIndex,
  variableCatalogToResourceMetas,
  variableRevisionsFromIndex,
} from '@/features/core/variable/variableCatalog';

import {
  buildGraphResourceMeta,
  resourceKey,
  type ProjectResourceMeta,
  type ResourceKey,
} from '@/features/core/resource';
import type { GraphMeta } from './graphMetaStore';
import type { WorksheetIndexEntry } from '@/shared/types/domain/worksheet';
import type { EditorTabMemento } from '@/features/core/layout/editorTabStore';
import type { LayoutTree } from '@/shared/types/ui';
import type { DetailFocus } from '@/features/core/editor/detail/types';
import {
  clearEditorGroupMaximizedHidden,
  listEditorGroupIds,
  writeEditorAreaMaximizeState,
} from '@/features/core/layout/editorGridLayout';
import { commitEditorGridLayoutState } from '@/features/core/layout/editorGridSizing';
import {
  createInitialWorkbenchNodes,
  DEFAULT_EDITOR_GROUP_ID,
  EDITOR_AREA_ID,
} from '@/features/core/layout/workbenchLayoutDefaults';
import { formatDisplayPath } from '@/shared/utils/formatDisplayPath';
import { LoadStatus } from '@/shared/types/ui/common';

export interface AuthoritativeProjectLoadSource {
  readonly path: string | null;
  readonly databases: Record<string, unknown>;
  readonly index: ProjectIndexRow;
}

interface PreparedLayoutState {
  readonly nodes: LayoutTree;
  readonly activeEditorGroupId: string;
  readonly recentEditorGroupIds: string[];
  readonly tabs: EditorTabMemento;
}

export interface PreparedAuthoritativeProjectLoad extends AuthoritativeProjectLoadSource {
  readonly projectData: ProjectData;
  readonly storeState: {
    readonly databases: Record<string, DatabaseRecord>;
    readonly databaseRevisions: Record<string, number>;
    readonly variables: Record<string, Variable>;
    readonly variableRevisions: Record<string, number>;
    readonly graphMeta: Record<string, GraphMeta>;
    readonly worksheetIndex: WorksheetIndexEntry[];
    readonly resources: Record<ResourceKey, ProjectResourceMeta>;
    readonly graphOrder: string[];
    readonly layout: PreparedLayoutState;
    readonly detailFocus: DetailFocus | null;
    readonly history: { canUndo: boolean; canRedo: boolean; pending: false };
    readonly projectIO: {
      projectInstanceId: string;
      status: LoadStatus.Ready;
      error: null;
      currentPath: string | null;
    };
  };
}

export interface AuthoritativeProjectLoadPlanContext {
  readonly databases: Record<string, DatabaseRecord>;
  readonly layoutNodes: LayoutTree;
  readonly editorTabs: EditorTabMemento;
  readonly recentEditorGroupIds: string[];
  readonly detailFocus: DetailFocus | null;
}

export interface AuthoritativeProjectLoadPlanDependencies {
  normalizeDatabases(
    raw: Record<string, unknown>,
    current: Record<string, DatabaseRecord>,
  ): Record<string, DatabaseRecord>;
  normalizeVariables(index: ProjectIndexRow): {
    variables: Record<string, Variable>;
    revisions: Record<string, number>;
  };
  prepareFunctionState(graphs: ProjectGraphIndexRow[]): Record<string, GraphMeta>;
  prepareResourceState(input: {
    graphs: ProjectGraphIndexRow[];
    worksheets: WorksheetIndexEntry[];
    variables: Record<string, Variable>;
    databases: Record<string, DatabaseRecord>;
  }): { resources: Record<ResourceKey, ProjectResourceMeta>; graphOrder: string[] };
  prepareLayoutState(
    context: AuthoritativeProjectLoadPlanContext,
    authoritativeWorksheetPaths: ReadonlySet<string>,
  ): PreparedLayoutState;
  validateCoordinatorStart(projectInstanceId: string, publicationRevision: number): void;
}

function prepareVariables(index: ProjectIndexRow) {
  const variables = applyVariableCatalogFromIndex(index.variables);
  return {
    variables,
    revisions: variableRevisionsFromIndex(index.variables),
  };
}

function prepareFunctionState(graphs: ProjectGraphIndexRow[]): Record<string, GraphMeta> {
  return Object.fromEntries(graphs.flatMap((graph) => {
    if (graph.type !== 'function') return [];
    return [[graph.path, {
      path: graph.path,
      name: graph.name,
      type: 'function' as const,
      functionRevision: graph.functionEditorProjection.functionRevision,
      functionSignature: structuredClone(graph.functionSignature),
      functionInputs: structuredClone(graph.functionEditorProjection.inputs),
      functionOutputs: structuredClone(graph.functionEditorProjection.outputs),
    }]];
  }));
}

export function buildProjectResourceState(input: {
  graphs: ProjectGraphIndexRow[];
  worksheets: WorksheetIndexEntry[];
  variables: Record<string, Variable>;
  databases: Record<string, DatabaseRecord>;
  loadedWorksheetPaths?: ReadonlySet<string>;
}): { resources: Record<ResourceKey, ProjectResourceMeta>; graphOrder: string[] } {
  const resources: ProjectResourceMeta[] = input.graphs.map((graph) =>
    buildGraphResourceMeta(graph.type, graph.path, graph.name, { revision: graph.revision }));
  for (const worksheet of input.worksheets) {
    resources.push({
      id: worksheet.worksheetPath,
      kind: 'worksheet',
      name: worksheet.name,
      uri: `yssbi://worksheet/${worksheet.worksheetPath}`,
      revision: worksheet.revision,
      exists: true,
      loaded: input.loadedWorksheetPaths?.has(worksheet.worksheetPath) ?? false,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
  }
  resources.push(...variableCatalogToResourceMetas(input.variables));
  for (const [id, database] of Object.entries(input.databases)) {
    resources.push({
      id,
      kind: 'database',
      name: typeof database.name === 'string' ? database.name : id,
      uri: `yssbi://database/${id}`,
      exists: true,
      loaded: true,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
  }
  return {
    resources: Object.fromEntries(
      resources.map((resource) => [resourceKey(resource), resource]),
    ) as Record<ResourceKey, ProjectResourceMeta>,
    graphOrder: input.graphs.map((graph) => graph.path),
  };
}

function collectDescendants(nodes: LayoutTree, rootId: string, skipId: string): string[] {
  const result: string[] = [];
  const visit = (id: string) => {
    if (id === skipId) return;
    const node = nodes[id];
    if (!node) return;
    result.push(id);
    node.children?.forEach(visit);
  };
  nodes[rootId]?.children?.forEach(visit);
  return result;
}

function prepareTabs(
  memento: EditorTabMemento,
  authoritativeWorksheetPaths: ReadonlySet<string>,
): EditorTabMemento {
  const tabs = structuredClone(memento);
  for (const [tabId, tab] of Object.entries(tabs.registry)) {
    const staleProjectTab = tab.type === 'event'
      || tab.type === 'function'
      || (tab.type === 'worksheet' && !authoritativeWorksheetPaths.has(tab.id));
    if (staleProjectTab) delete tabs.registry[tabId];
  }

  const mergedIds: string[] = [];
  const selectedTabIds: string[] = [];
  const seenTabs = new Set<string>();
  const seenSelectedTabs = new Set<string>();
  let activeTabId: string | null = null;
  for (const placement of Object.values(tabs.placements)) {
    for (const tabId of placement.tabIds) {
      if (!tabs.registry[tabId] || seenTabs.has(tabId)) continue;
      seenTabs.add(tabId);
      mergedIds.push(tabId);
    }
    if (placement.activeTabId && tabs.registry[placement.activeTabId]) {
      activeTabId ??= placement.activeTabId;
    }

    for (const tabId of placement.selectedTabIds) {
      if (!tabs.registry[tabId] || seenSelectedTabs.has(tabId)) continue;
      seenSelectedTabs.add(tabId);
      selectedTabIds.push(tabId);
    }
  }
  tabs.placements = mergedIds.length === 0 ? {} : {
    [DEFAULT_EDITOR_GROUP_ID]: {
      tabIds: mergedIds,
      activeTabId: activeTabId ?? mergedIds[mergedIds.length - 1] ?? null,
      selectedNodeIds: [],
      selectedConnectionIds: [],
      selectedTabIds,
    },
  };
  return tabs;
}

function prepareLayout(
  context: AuthoritativeProjectLoadPlanContext,
  authoritativeWorksheetPaths: ReadonlySet<string>,
): PreparedLayoutState {
  const nodes = structuredClone(context.layoutNodes);
  const editorArea = nodes[EDITOR_AREA_ID];
  if (!editorArea?.children) throw new Error('Project layout is missing the editor area');
  for (const id of collectDescendants(nodes, EDITOR_AREA_ID, DEFAULT_EDITOR_GROUP_ID)) {
    delete nodes[id];
  }
  const defaultEditor = nodes[DEFAULT_EDITOR_GROUP_ID]
    ?? structuredClone(createInitialWorkbenchNodes()[DEFAULT_EDITOR_GROUP_ID]);
  nodes[DEFAULT_EDITOR_GROUP_ID] = defaultEditor;
  editorArea.children = [DEFAULT_EDITOR_GROUP_ID];
  writeEditorAreaMaximizeState(nodes, null, null);
  clearEditorGroupMaximizedHidden(nodes);
  defaultEditor.parentId = EDITOR_AREA_ID;
  defaultEditor.size = 1;
  defaultEditor.pixelSize = undefined;
  defaultEditor.data = { ...defaultEditor.data, component: 'GraphEditor' };
  commitEditorGridLayoutState(nodes);
  return {
    nodes,
    activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
    recentEditorGroupIds: [
      DEFAULT_EDITOR_GROUP_ID,
      ...context.recentEditorGroupIds.filter((id) =>
        id !== DEFAULT_EDITOR_GROUP_ID && listEditorGroupIds(nodes).includes(id)),
    ],
    tabs: prepareTabs(context.editorTabs, authoritativeWorksheetPaths),
  };
}

export const defaultAuthoritativeProjectLoadPlanDependencies: Omit<
  AuthoritativeProjectLoadPlanDependencies,
  'validateCoordinatorStart'
> = {
  normalizeDatabases,
  normalizeVariables: prepareVariables,
  prepareFunctionState,
  prepareResourceState: buildProjectResourceState,
  prepareLayoutState: prepareLayout,
};

export function buildAuthoritativeProjectLoadPlan(
  source: AuthoritativeProjectLoadSource,
  context: AuthoritativeProjectLoadPlanContext,
  dependencies: AuthoritativeProjectLoadPlanDependencies,
): PreparedAuthoritativeProjectLoad {
  const normalizedDatabases = dependencies.normalizeDatabases(source.databases, context.databases);
  const databaseRows = source.index.databases;
  const databaseResourcePaths = Object.fromEntries(
    databaseRows.map((row) => [row.id, row.resourcePath]),
  );
  const databaseRevisions = Object.fromEntries(
    databaseRows.map((row) => [row.id, row.revision]),
  );
  const databases = Object.fromEntries(Object.entries(normalizedDatabases).map(([id, database]) => [
    id,
    { ...database, resourcePath: databaseResourcePaths[id] },
  ]));
  const variableState = dependencies.normalizeVariables(source.index);
  const worksheetIndex = source.index.worksheets.map((worksheet) => ({
    worksheetPath: worksheet.worksheetPath,
    name: worksheet.name,
    databaseId: worksheet.databaseId,
    chartType: worksheet.chartType as WorksheetIndexEntry['chartType'],
    revision: worksheet.revision,
  }));
  const graphMeta = dependencies.prepareFunctionState(source.index.graphs);
  const resourceState = dependencies.prepareResourceState({
    graphs: source.index.graphs,
    worksheets: worksheetIndex,
    variables: variableState.variables,
    databases,
  });
  const authoritativeWorksheetPaths = new Set(
    worksheetIndex.map((worksheet) => worksheet.worksheetPath),
  );
  const layout = dependencies.prepareLayoutState(context, authoritativeWorksheetPaths);
  const detailFocus = context.detailFocus?.kind === 'worksheet'
    && authoritativeWorksheetPaths.has(context.detailFocus.worksheetPath)
    ? structuredClone(context.detailFocus)
    : null;
  dependencies.validateCoordinatorStart(
    source.index.projectInstanceId,
    source.index.publicationRevision,
  );
  const projectData = {
    variables: variableState.variables,
    databases: source.databases,
    graphs: {},
    metadata: {
      exportTime: source.index.exportTime,
    },
  } as ProjectData;
  return {
    ...source,
    projectData,
    storeState: {
      databases,
      databaseRevisions,
      variables: variableState.variables,
      variableRevisions: variableState.revisions,
      graphMeta,
      worksheetIndex,
      resources: resourceState.resources,
      graphOrder: resourceState.graphOrder,
      layout,
      detailFocus,
      history: {
        canUndo: source.index.history.canUndo,
        canRedo: source.index.history.canRedo,
        pending: false,
      },
      projectIO: {
        projectInstanceId: source.index.projectInstanceId,
        status: LoadStatus.Ready,
        error: null,
        currentPath: source.path ? formatDisplayPath(source.path) : null,
      },
    },
  };
}
