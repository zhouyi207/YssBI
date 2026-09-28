import type { FileResourceKind, ResourceKind } from "@/shared/types/domain/resource";
import type { ResourceRef } from "@/features/core/resource";
import { resourceKey, useResourceStore } from "@/features/core/resource";
import { mindActions } from "./mindActions";
import { docActions } from "./docActions";
import { saveGraph } from "@/features/application/graphEditing/saveGraph";
import { enqueueGraphTask } from "@/features/application/graphEditing/graphEditCoordinator";
import { saveChartDocument } from "@/features/application/chart/saveChartDocument";
import {
  DEFAULT_EVENT_GRAPH_NAME,
  DEFAULT_FUNCTION_GRAPH_NAME,
  DEFAULT_CHART_NAME,
  DEFAULT_MIND_NAME,
  DEFAULT_DOC_NAME,
} from "@/shared/constants/defaultResourceNames";
import { EventGraphService, FunctionGraphService } from "@/services/project/fileResourceService";
import { ChartService } from "@/services/chart/chartService";
import { DatabaseService } from "@/services/database/databaseService";
import { executeDatabaseMutation } from "@/features/application/dataManagement/databaseMutation";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import {
  captureProjectCommandContext,
  type ProjectCommandContext,
} from "@/features/application/projectCommandContext";
import { beginGraphRenameLifecycle } from "@/features/application/graphProjection/graphProjectionLifecycle";
import {
  beginChartRenameLifecycle,
  isChartLifecycleCurrent,
} from "@/features/application/editor/chartLifecycleCoordinator";
import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";

export type FileResourceRef = { id: string; kind: FileResourceKind };
export interface FileResourceHandler {
  readonly categoryId: string;
  create(name?: string): Promise<string>;
  rename(path: string, name: string): Promise<void>;
  duplicate(path: string): Promise<string>;
  remove(path: string): Promise<void>;
  save(path: string): Promise<boolean>;
  settle(path: string): Promise<unknown>;
}

async function settleGraphEdits(path: string): Promise<void> {
  if (!(await enqueueGraphTask(path, async () => true, false)))
    throw new Error("Graph editing lifecycle changed");
}

function resourceRevision(ref: ResourceRef): number {
  const revision = useResourceStore.getState().resources[resourceKey(ref)]?.revision;
  if (revision == null) throw new Error(`Resource '${ref.id}' has no authoritative revision`);
  return revision;
}
async function publish(
  operation: (context: ProjectCommandContext) => Promise<ResourceMutationResultDto>,
): Promise<ResourceMutationResultDto> {
  const context = captureProjectCommandContext();
  const result = await operation(context);
  context.assertCurrent();
  if (result.projectInstanceId !== context.projectInstanceId)
    throw new Error("stale project lifecycle for resource mutation");
  await projectPublicationCoordinator.submit({ result });
  context.assertCurrent();
  return result;
}
function createdPath(result: ResourceMutationResultDto, kind: FileResourceKind): string {
  for (const delta of result.deltas) {
    if (
      delta.payload.kind === "resource_lifecycle" &&
      delta.payload.patch.before === null &&
      delta.payload.patch.after?.kind === kind
    )
      return delta.payload.patch.after.path;
  }
  throw new Error(`Resource mutation omitted its created ${kind} file`);
}
async function renameNodeFile(
  ref: FileResourceRef,
  name: string,
  service: typeof EventGraphService,
): Promise<void> {
  const revision = resourceRevision(ref);
  const lifecycleToken = beginGraphRenameLifecycle(ref.id);
  await publish((context) =>
    service.rename(
      context.projectInstanceId,
      context.operationId,
      ref.id,
      revision,
      name,
      lifecycleToken,
    ),
  );
}
async function duplicateNodeFile(
  ref: FileResourceRef,
  service: typeof EventGraphService,
): Promise<string> {
  return createdPath(
    await publish((context) =>
      service.duplicate(
        context.projectInstanceId,
        context.operationId,
        ref.id,
        resourceRevision(ref),
      ),
    ),
    ref.kind,
  );
}
async function removeNodeFile(
  ref: FileResourceRef,
  service: typeof EventGraphService,
): Promise<void> {
  await publish((context) =>
    service.remove(context.projectInstanceId, context.operationId, ref.id, resourceRevision(ref)),
  );
}
async function duplicateAuthoredFile(
  path: string,
  duplicate: (path: string) => Promise<{ path: string } | null>,
): Promise<string> {
  const snapshot = await duplicate(path);
  if (!snapshot) throw new Error("Missing duplicated file");
  return snapshot.path;
}

// Each file type owns its choices. Common helpers perform transactions and publication only.
export const fileResourceHandlers = {
  event_graph: {
    settle: settleGraphEdits,
    categoryId: "project.eventGraphs",
    create: async (name) =>
      createdPath(
        await publish((context) =>
          EventGraphService.create(
            context.projectInstanceId,
            context.operationId,
            name?.trim() || DEFAULT_EVENT_GRAPH_NAME,
          ),
        ),
        "event_graph",
      ),
    rename: (id, name) => renameNodeFile({ id, kind: "event_graph" }, name, EventGraphService),
    duplicate: (id) => duplicateNodeFile({ id, kind: "event_graph" }, EventGraphService),
    remove: (id) => removeNodeFile({ id, kind: "event_graph" }, EventGraphService),
    save: (path) => saveGraph(path, "event_graph"),
  },
  function_graph: {
    settle: settleGraphEdits,
    categoryId: "project.functionGraphs",
    create: async (name) =>
      createdPath(
        await publish((context) =>
          FunctionGraphService.create(
            context.projectInstanceId,
            context.operationId,
            name?.trim() || DEFAULT_FUNCTION_GRAPH_NAME,
          ),
        ),
        "function_graph",
      ),
    rename: (id, name) =>
      renameNodeFile({ id, kind: "function_graph" }, name, FunctionGraphService),
    duplicate: (id) => duplicateNodeFile({ id, kind: "function_graph" }, FunctionGraphService),
    remove: (id) => removeNodeFile({ id, kind: "function_graph" }, FunctionGraphService),
    save: (path) => saveGraph(path, "function_graph"),
  },
  chart: {
    settle: async () => {},
    categoryId: "project.charts",
    create: async (name) =>
      createdPath(
        await publish((context) =>
          ChartService.createChart(
            context.projectInstanceId,
            context.operationId,
            name?.trim() || DEFAULT_CHART_NAME,
          ),
        ),
        "chart",
      ),
    rename: async (id, name) => {
      const revision = resourceRevision({ id, kind: "chart" });
      await publish(async (context) => {
        const lifecycleToken = beginChartRenameLifecycle(context.projectInstanceId, id);
        const result = await ChartService.renameChart(
          context.projectInstanceId,
          context.operationId,
          id,
          revision,
          name,
          lifecycleToken,
        );
        context.assertCurrent();
        if (!isChartLifecycleCurrent(context.projectInstanceId, id, lifecycleToken))
          throw Object.assign(new Error("stale chart lifecycle"), {
            code: "stale_resource_lifecycle",
          });
        return result;
      });
    },
    duplicate: async (id) =>
      createdPath(
        await publish((context) =>
          ChartService.duplicateChart(
            context.projectInstanceId,
            context.operationId,
            id,
            resourceRevision({ id, kind: "chart" }),
          ),
        ),
        "chart",
      ),
    remove: async (id) => {
      await publish((context) =>
        ChartService.removeChart(
          context.projectInstanceId,
          context.operationId,
          id,
          resourceRevision({ id, kind: "chart" }),
        ),
      );
    },
    save: saveChartDocument,
  },
  mind: {
    settle: mindActions.barrier,
    categoryId: "project.minds",
    create: (name) => mindActions.create(name?.trim() || DEFAULT_MIND_NAME),
    rename: async (path, name) => {
      await mindActions.rename(path, name);
    },
    duplicate: (path) => duplicateAuthoredFile(path, mindActions.duplicate),
    remove: async (path) => {
      await mindActions.remove(path);
    },
    save: mindActions.save,
  },
  doc: {
    settle: docActions.barrier,
    categoryId: "project.docs",
    create: (name) => docActions.create(name?.trim() || DEFAULT_DOC_NAME),
    rename: async (path, name) => {
      await docActions.rename(path, name);
    },
    duplicate: (path) => duplicateAuthoredFile(path, docActions.duplicate),
    remove: async (path) => {
      await docActions.remove(path);
    },
    save: docActions.save,
  },
} satisfies Record<FileResourceKind, FileResourceHandler>;

const resourceHandlers = {
  ...fileResourceHandlers,
  database: {
    rename: async (id: string, name: string) => {
      await executeDatabaseMutation(id, (authority) =>
        DatabaseService.renameDatabase(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          id,
          name,
        ),
      );
    },
    remove: async (id: string) => {
      await executeDatabaseMutation(id, (authority) =>
        DatabaseService.deleteDatabase(
          authority.projectInstanceId,
          authority.operationId,
          authority.expectedRevision,
          id,
        ),
      );
    },
  },
} satisfies Record<ResourceKind, Pick<FileResourceHandler, "rename" | "remove">>;

export function createFileResource(kind: FileResourceKind, name?: string): Promise<string> {
  return fileResourceHandlers[kind].create(name);
}
export function duplicateFileResource(ref: FileResourceRef): Promise<string> {
  return fileResourceHandlers[ref.kind].duplicate(ref.id);
}
export function saveFileResource(path: string, kind: FileResourceKind): Promise<boolean> {
  return fileResourceHandlers[kind].save(path);
}
export function renameResource(ref: ResourceRef, name: string): Promise<void> {
  return name ? resourceHandlers[ref.kind].rename(ref.id, name) : Promise.resolve();
}
export function deleteResource(ref: ResourceRef): Promise<void> {
  return resourceHandlers[ref.kind].remove(ref.id);
}
