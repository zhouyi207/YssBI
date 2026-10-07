import { nodeFileEntries } from "@/shared/types/domain/project";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import { prepareDatabaseIndexSnapshot } from "@/features/application/dataManagement/databaseRecords";
import type { ProjectDatabaseMetadata } from "@/services/database/databaseWireParser";
import type { ProjectIndexRow } from "@/shared/types/domain/project";

import {
  buildFileResourceMeta,
  resourceKey,
  type ProjectResourceMeta,
  type ResourceKey,
} from "@/features/core/resource";
import { prepareGraphMetaSnapshot, type GraphMeta } from "@/features/core/dataStore/graphMeta";
import type { DetailFocus } from "@/features/core/editor/detail/detailTypes";
import { detailResource } from "@/features/core/editor/detail/editorDetailPolicy";
import { LoadStatus } from "@/shared/types/ui/common";

export interface AuthoritativeProjectLoadSource {
  readonly path: string | null;
  readonly databases: Readonly<Record<string, ProjectDatabaseMetadata>>;
  readonly index: ProjectIndexRow;
}

export interface PreparedAuthoritativeProjectLoad extends AuthoritativeProjectLoadSource {
  readonly storeState: {
    readonly databases: Record<string, DatabaseRecord>;
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
  readonly resources: Record<ResourceKey, ProjectResourceMeta>;
  readonly detailFocus: DetailFocus | null;
}

export interface AuthoritativeProjectLoadPlanDependencies {
  prepareDatabases: typeof prepareDatabaseIndexSnapshot;
  prepareFunctionState(
    functionGraphs: ProjectIndexRow["functionGraphs"],
  ): Record<string, GraphMeta>;
  prepareResourceState(input: {
    eventGraphs: ProjectIndexRow["eventGraphs"];
    functionGraphs: ProjectIndexRow["functionGraphs"];
    charts: ProjectIndexRow["charts"];
    minds: ProjectIndexRow["minds"];
    docs: ProjectIndexRow["docs"];
    databases: ProjectIndexRow["databases"];
  }): { resources: Record<ResourceKey, ProjectResourceMeta>; graphOrder: string[] };

  validateCoordinatorStart(projectInstanceId: string, publicationRevision: number): void;
}

export function buildProjectResourceState(input: {
  eventGraphs: ProjectIndexRow["eventGraphs"];
  functionGraphs: ProjectIndexRow["functionGraphs"];
  charts: ProjectIndexRow["charts"];
  minds: ProjectIndexRow["minds"];
  docs: ProjectIndexRow["docs"];
  databases: ProjectIndexRow["databases"];
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
  for (const database of input.databases) {
    const id = database.id;
    resources.push({
      id,
      kind: "database",
      name: typeof database.name === "string" ? database.name : id,
      uri: resourceKey({ id, kind: "database" }),
      resourcePath: database.resourcePath,
      revision: database.revision,
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
  prepareDatabases: prepareDatabaseIndexSnapshot,
  prepareFunctionState: prepareGraphMetaSnapshot,
  prepareResourceState: buildProjectResourceState,
};

export function buildAuthoritativeProjectLoadPlan(
  source: AuthoritativeProjectLoadSource,
  context: AuthoritativeProjectLoadPlanContext,
  dependencies: AuthoritativeProjectLoadPlanDependencies,
): PreparedAuthoritativeProjectLoad {
  const databases = dependencies.prepareDatabases(
    source.index.databases,
    context.databases,
    context.resources,
    source.databases,
  );
  const graphMeta = dependencies.prepareFunctionState(source.index.functionGraphs);
  const resourceState = dependencies.prepareResourceState({
    eventGraphs: source.index.eventGraphs,
    functionGraphs: source.index.functionGraphs,
    charts: source.index.charts,
    minds: source.index.minds,
    docs: source.index.docs,
    databases: source.index.databases,
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
