import { nodeFileEntries } from "@/shared/types/domain/project";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import { normalizeDatabases } from "@/features/application/dataManagement/databaseRecords";
import type { ProjectIndexRow } from "@/shared/types/domain/project";

import {
  buildFileResourceMeta,
  resourceKey,
  type ProjectResourceMeta,
  type ResourceKey,
} from "@/features/core/resource";
import type { GraphMeta } from "@/features/core/dataStore/graphMetaStore";
import type { DetailFocus } from "@/features/core/editor/detail/detailTypes";
import { detailResource } from "@/features/core/editor/detail/editorDetailPolicy";
import { LoadStatus } from "@/shared/types/ui/common";

export interface AuthoritativeProjectLoadSource {
  readonly path: string | null;
  readonly databases: Record<string, unknown>;
  readonly index: ProjectIndexRow;
}

export interface PreparedAuthoritativeProjectLoad extends AuthoritativeProjectLoadSource {
  readonly storeState: {
    readonly databases: Record<string, DatabaseRecord>;
    readonly databaseRevisions: Record<string, number>;
    readonly graphMeta: Record<string, GraphMeta>;
    readonly resources: Record<ResourceKey, ProjectResourceMeta>;
    readonly graphOrder: string[];
    readonly detailFocus: DetailFocus | null;
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
  prepareFunctionState(
    functionGraphs: ProjectIndexRow["functionGraphs"],
  ): Record<string, GraphMeta>;
  prepareResourceState(input: {
    eventGraphs: ProjectIndexRow["eventGraphs"];
    functionGraphs: ProjectIndexRow["functionGraphs"];
    charts: ProjectIndexRow["charts"];
    minds: ProjectIndexRow["minds"];
    docs: ProjectIndexRow["docs"];
    databases: Record<string, DatabaseRecord>;
  }): { resources: Record<ResourceKey, ProjectResourceMeta>; graphOrder: string[] };

  validateCoordinatorStart(projectInstanceId: string, publicationRevision: number): void;
}

function prepareFunctionState(
  functionGraphs: ProjectIndexRow["functionGraphs"],
): Record<string, GraphMeta> {
  return Object.fromEntries(
    functionGraphs.flatMap((graph) => {
      return [
        [
          graph.path,
          {
            path: graph.path,
            name: graph.name,
            type: "function_graph" as const,
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
  eventGraphs: ProjectIndexRow["eventGraphs"];
  functionGraphs: ProjectIndexRow["functionGraphs"];
  charts: ProjectIndexRow["charts"];
  minds: ProjectIndexRow["minds"];
  docs: ProjectIndexRow["docs"];
  databases: Record<string, DatabaseRecord>;
  loadedChartPaths?: ReadonlySet<string>;
}): { resources: Record<ResourceKey, ProjectResourceMeta>; graphOrder: string[] } {
  const resources: ProjectResourceMeta[] = nodeFileEntries(input).map((graph) =>
    buildFileResourceMeta(graph.type, graph.path, graph.name, { revision: graph.revision }),
  );
  for (const document of [...input.minds, ...input.docs]) {
    resources.push({
      id: document.path,
      kind: document.kind,
      name: document.name,
      uri: resourceKey({ id: document.path, kind: document.kind }),
      revision: document.revision,
      exists: true,
      loaded: false,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
  }
  for (const chart of input.charts) {
    resources.push({
      id: chart.chartPath,
      kind: "chart",
      name: chart.name,
      uri: resourceKey({ id: chart.chartPath, kind: "chart" }),
      revision: chart.revision,
      exists: true,
      loaded: input.loadedChartPaths?.has(chart.chartPath) ?? false,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
  }
  for (const [id, database] of Object.entries(input.databases)) {
    resources.push({
      id,
      kind: "database",
      name: typeof database.name === "string" ? database.name : id,
      uri: resourceKey({ id, kind: "database" }),
      resourcePath: database.resourcePath,
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
    graphOrder: nodeFileEntries(input).map((graph) => graph.path),
  };
}

export const defaultAuthoritativeProjectLoadPlanDependencies: Omit<
  AuthoritativeProjectLoadPlanDependencies,
  "validateCoordinatorStart"
> = {
  normalizeDatabases,
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
  const graphMeta = dependencies.prepareFunctionState(source.index.functionGraphs);
  const resourceState = dependencies.prepareResourceState({
    eventGraphs: source.index.eventGraphs,
    functionGraphs: source.index.functionGraphs,
    charts: source.index.charts,
    minds: source.index.minds,
    docs: source.index.docs,
    databases,
  });
  const focus = context.detailFocus;
  const focusedResource = detailResource(focus, resourceState.resources);
  const detailFocus =
    focusedResource && resourceState.resources[resourceKey(focusedResource)]?.exists
      ? structuredClone(focus)
      : null;
  dependencies.validateCoordinatorStart(
    source.index.projectInstanceId,
    source.index.publicationRevision,
  );
  return {
    ...source,
    storeState: {
      databases,
      databaseRevisions,
      graphMeta,
      resources: resourceState.resources,
      graphOrder: resourceState.graphOrder,
      detailFocus,
      projectIO: {
        projectInstanceId: source.index.projectInstanceId,
        status: LoadStatus.Ready,
        error: null,
        currentPath: source.path || null,
      },
    },
  };
}
