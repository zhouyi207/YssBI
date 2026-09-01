import type { ProjectData, Variable } from "@/shared/types";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import { normalizeDatabases } from "@/features/application/dataManagement/databaseRecords";
import type { ProjectGraphIndexRow, ProjectIndexRow } from "@/shared/types/domain/project";
import {
  applyVariableCatalogFromIndex,
  variableCatalogToResourceMetas,
  variableRevisionsFromIndex,
} from "@/features/core/variable/variableCatalog";

import {
  buildGraphResourceMeta,
  resourceKey,
  type ProjectResourceMeta,
  type ResourceKey,
} from "@/features/core/resource";
import type { GraphMeta } from "@/features/core/dataStore/graphMetaStore";
import type { ChartIndexEntry } from "@/shared/types/domain/chart";
import type { DetailFocus } from "@/shared/types/ui/detail";
import { formatDisplayPath } from "@/shared/utils/formatDisplayPath";
import { LoadStatus } from "@/shared/types/ui/common";

export interface AuthoritativeProjectLoadSource {
  readonly path: string | null;
  readonly databases: Record<string, unknown>;
  readonly index: ProjectIndexRow;
}

export interface PreparedAuthoritativeProjectLoad extends AuthoritativeProjectLoadSource {
  readonly projectData: ProjectData;
  readonly storeState: {
    readonly databases: Record<string, DatabaseRecord>;
    readonly databaseRevisions: Record<string, number>;
    readonly variables: Record<string, Variable>;
    readonly variableRevisions: Record<string, number>;
    readonly graphMeta: Record<string, GraphMeta>;
    readonly chartIndex: ChartIndexEntry[];
    readonly resources: Record<ResourceKey, ProjectResourceMeta>;
    readonly graphOrder: string[];
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
    charts: ChartIndexEntry[];
    variables: Record<string, Variable>;
    databases: Record<string, DatabaseRecord>;
  }): { resources: Record<ResourceKey, ProjectResourceMeta>; graphOrder: string[] };

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
  return Object.fromEntries(
    graphs.flatMap((graph) => {
      if (graph.type !== "function") return [];
      return [
        [
          graph.path,
          {
            path: graph.path,
            name: graph.name,
            type: "function" as const,
            functionRevision: graph.functionEditorProjection.functionRevision,
            functionSignature: structuredClone(graph.functionSignature),
            functionInputs: structuredClone(graph.functionEditorProjection.inputs),
            functionOutputs: structuredClone(graph.functionEditorProjection.outputs),
          },
        ],
      ];
    }),
  );
}

export function buildProjectResourceState(input: {
  graphs: ProjectGraphIndexRow[];
  charts: ChartIndexEntry[];
  variables: Record<string, Variable>;
  databases: Record<string, DatabaseRecord>;
  loadedChartPaths?: ReadonlySet<string>;
}): { resources: Record<ResourceKey, ProjectResourceMeta>; graphOrder: string[] } {
  const resources: ProjectResourceMeta[] = input.graphs.map((graph) =>
    buildGraphResourceMeta(graph.type, graph.path, graph.name, { revision: graph.revision }),
  );
  for (const chart of input.charts) {
    resources.push({
      id: chart.chartPath,
      kind: "chart",
      name: chart.name,
      uri: `yssbi://chart/${chart.chartPath}`,
      revision: chart.revision,
      exists: true,
      loaded: input.loadedChartPaths?.has(chart.chartPath) ?? false,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
  }
  resources.push(...variableCatalogToResourceMetas(input.variables));
  for (const [id, database] of Object.entries(input.databases)) {
    resources.push({
      id,
      kind: "database",
      name: typeof database.name === "string" ? database.name : id,
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

export const defaultAuthoritativeProjectLoadPlanDependencies: Omit<
  AuthoritativeProjectLoadPlanDependencies,
  "validateCoordinatorStart"
> = {
  normalizeDatabases,
  normalizeVariables: prepareVariables,
  prepareFunctionState,
  prepareResourceState: buildProjectResourceState,
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
  const databaseRevisions = Object.fromEntries(databaseRows.map((row) => [row.id, row.revision]));
  const databases = Object.fromEntries(
    Object.entries(normalizedDatabases).map(([id, database]) => [
      id,
      { ...database, resourcePath: databaseResourcePaths[id] },
    ]),
  );
  const variableState = dependencies.normalizeVariables(source.index);
  const chartIndex = source.index.charts.map((chart) => ({
    chartPath: chart.chartPath,
    name: chart.name,
    databaseId: chart.databaseId,
    chartType: chart.chartType as ChartIndexEntry["chartType"],
    revision: chart.revision,
  }));
  const graphMeta = dependencies.prepareFunctionState(source.index.graphs);
  const resourceState = dependencies.prepareResourceState({
    graphs: source.index.graphs,
    charts: chartIndex,
    variables: variableState.variables,
    databases,
  });
  const authoritativeChartPaths = new Set(chartIndex.map((chart) => chart.chartPath));

  const detailFocus =
    context.detailFocus?.kind === "chart" &&
    authoritativeChartPaths.has(context.detailFocus.chartPath)
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
      chartIndex,
      resources: resourceState.resources,
      graphOrder: resourceState.graphOrder,
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
