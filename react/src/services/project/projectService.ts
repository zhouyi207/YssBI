import { isMindPath, type MindIndexEntry } from "@/shared/types/domain/mind";
import { isDocPath, type DocIndexEntry } from "@/shared/types/domain/doc";
import {
  PROJECT_ACTIVITY_PANEL_IDS,
  type ActivityPanelSnapshot,
  type ProjectActivityPanelId,
} from "@/shared/types/domain/activityPanel";
import type { ProjectIndexSnapshot } from "@/shared/types/domain/project";
import { parseActivityPanelResponse } from "@/shared/types/dto/activityPanel";
import { DEFAULT_LANGUAGE } from "@/shared/types/settings";
import { Channel } from "@tauri-apps/api/core";
import type { ZodType } from "zod";
import { clearChannelMessageHandler } from "@/shared/platform/tauriWebview";
import { logger } from "@/utils/frontendLogger";
import type { RunEvent } from "@/shared/types/dto/runEvent";
import type { ExecutionDemandDto } from "@/shared/types/dto/executionDemand";
import { parseExecutionDemandDto } from "@/shared/types/dto/runEventParser";

import type {
  ProjectDatabaseIndexRow,
  ProjectEventGraphIndexRow,
  ProjectFunctionGraphIndexRow,
  ProjectIndexRow,
  ProjectChartIndexRow,
} from "@/shared/types/domain/project";
import {
  isFunctionSignatureDto,
  type GraphEditVersionDto,
} from "@/shared/types/domain/editorMutation";
import { isDatabaseEngine } from "@/shared/types/domain/database";
import { projectDatabasesSchema } from "@/services/database/databaseWireParser";
import { isChartType } from "@/shared/types/domain/chart";
import {
  isFunctionEditorProjectionDto,
  isGraphResourcePath,
} from "@/shared/types/dto/editorProjectionGuards";
import type {
  CleanupInvalidProjectsResult,
  LifecycleMutationResultDto,
  ProjectRecordRow,
  ProjectActivationResult,
  ScanProjectsResult,
} from "@/shared/types/dto/project";

import { trackChannel, untrackChannel } from "@/services/devHmrIpc";
import { IpcError, invokeCommand, isIpcErrorCode } from "@/services/ipc";
import { bindExecutionEventChannel } from "./executionChannelDrain";
import { readExecutionSnapshot } from "@/services/nodeSystem/graphActivityService";
import {
  parseProjectActivationResult,
  parseLifecycleMutationResult,
  parseProjectRecord,
  parseProjectRecords,
  parseScanProjectsResult,
  projectCleanupResultSchema,
  projectScanProgressSchema,
  projectCleanupProgressSchema,
  projectPathSchema,
  nullableProjectPathSchema,
  projectFlagSchema,
  type ProjectScanProgressEvent,
  type ProjectCleanupProgressEvent,
} from "./projectWireParser";

function projectProgressChannel<T>(schema: ZodType<T>, onProgress?: (event: T) => void) {
  const channel = trackChannel(new Channel<unknown>());
  channel.onmessage = (value) => {
    const parsed = schema.safeParse(value);
    if (parsed.success) onProgress?.(parsed.data);
    else logger.sys.warn("Ignored invalid project picker progress");
  };
  return channel;
}

export interface ExecuteGraphDocumentRequest {
  projectInstanceId: string;
  graphPath: string;
  version: GraphEditVersionDto;
  semanticInputHash: string;
  demand: ExecutionDemandDto;
  onEvent?: (event: RunEvent) => void;
}

export const PICKER_TASK_CANCELLED = "picker_task_cancelled";

export function isPickerTaskCancelledError(error: unknown): boolean {
  return isIpcErrorCode(error, PICKER_TASK_CANCELLED);
}

function commandSentTerminalRunEvent(error: unknown): boolean {
  return error instanceof IpcError && error.details?.terminalRunEventSent === true;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return (
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
  );
}

function isSafeRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function isNodeFileIndexBase(value: Record<string, unknown>): boolean {
  return (
    isGraphResourcePath(value.path) &&
    typeof value.name === "string" &&
    isSafeRevision(value.revision)
  );
}
export function parseProjectEventGraphIndexRow(value: unknown): ProjectEventGraphIndexRow {
  if (
    !isRecord(value) ||
    value.type !== "event_graph" ||
    !isNodeFileIndexBase(value) ||
    !hasExactKeys(value, ["path", "name", "type", "revision"])
  )
    throw new Error("Invalid project event index row");
  return value as unknown as ProjectEventGraphIndexRow;
}
export function parseProjectFunctionGraphIndexRow(value: unknown): ProjectFunctionGraphIndexRow {
  if (
    !isRecord(value) ||
    value.type !== "function_graph" ||
    !isNodeFileIndexBase(value) ||
    !hasExactKeys(value, [
      "path",
      "name",
      "type",
      "revision",
      "functionRevision",
      "functionSignature",
      "functionEditorProjection",
    ]) ||
    !isSafeRevision(value.functionRevision) ||
    !isFunctionSignatureDto(value.functionSignature) ||
    !isFunctionEditorProjectionDto(value.functionEditorProjection) ||
    value.functionEditorProjection.functionRevision !== value.functionRevision
  )
    throw new Error("Invalid project function index row");
  return value as unknown as ProjectFunctionGraphIndexRow;
}

function parseProjectChartIndexRow(value: unknown): ProjectChartIndexRow {
  if (
    !isRecord(value) ||
    Array.isArray(value) ||
    !hasExactKeys(value, ["chartPath", "name", "databaseId", "chartType", "revision"]) ||
    typeof value.chartPath !== "string" ||
    value.chartPath.length === 0 ||
    typeof value.name !== "string" ||
    value.name.trim().length === 0 ||
    typeof value.databaseId !== "string" ||
    !isChartType(value.chartType) ||
    !isSafeRevision(value.revision)
  ) {
    throw new Error("Invalid project chart index row");
  }
  return value as unknown as ProjectChartIndexRow;
}

function parseFileIndexEntry<K extends "mind" | "doc">(
  value: unknown,
  kind: K,
): Extract<MindIndexEntry | DocIndexEntry, { kind: K }> {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["path", "kind", "name", "revision"]) ||
    value.kind !== kind ||
    !(kind === "mind" ? isMindPath(value.path) : isDocPath(value.path)) ||
    typeof value.name !== "string" ||
    !value.name ||
    !isSafeRevision(value.revision)
  )
    throw new Error("Invalid document index entry");
  return value as unknown as Extract<MindIndexEntry | DocIndexEntry, { kind: K }>;
}

export function parseProjectIndexRow(value: unknown): ProjectIndexRow {
  if (
    !isRecord(value) ||
    Array.isArray(value) ||
    !hasExactKeys(value, [
      "projectInstanceId",
      "publicationRevision",
      "projectName",
      "exportTime",
      "eventGraphs",
      "functionGraphs",
      "charts",
      "minds",
      "docs",
      "databases",
    ]) ||
    typeof value.projectInstanceId !== "string" ||
    !isSafeRevision(value.publicationRevision) ||
    typeof value.projectName !== "string" ||
    typeof value.exportTime !== "string" ||
    !Array.isArray(value.eventGraphs) ||
    !Array.isArray(value.functionGraphs) ||
    !Array.isArray(value.charts) ||
    !Array.isArray(value.minds) ||
    !Array.isArray(value.docs) ||
    !Array.isArray(value.databases) ||
    !value.databases.every(isProjectDatabaseIndexRow)
  ) {
    throw new Error("Invalid project index response");
  }
  try {
    const eventGraphs = value.eventGraphs.map(parseProjectEventGraphIndexRow);
    const functionGraphs = value.functionGraphs.map(parseProjectFunctionGraphIndexRow);
    const charts = value.charts.map(parseProjectChartIndexRow);
    const minds = value.minds.map((value) => parseFileIndexEntry(value, "mind"));
    const docs = value.docs.map((value) => parseFileIndexEntry(value, "doc"));
    if (
      new Set(minds.map((file) => file.path)).size !== minds.length ||
      new Set(docs.map((file) => file.path)).size !== docs.length ||
      new Set(eventGraphs.map((file) => file.path)).size !== eventGraphs.length ||
      new Set(functionGraphs.map((file) => file.path)).size !== functionGraphs.length ||
      new Set(charts.map((chart) => chart.chartPath)).size !== charts.length ||
      new Set(value.databases.map((database) => database.id)).size !== value.databases.length
    )
      throw new Error("Duplicate project resource identity");
    return {
      projectInstanceId: value.projectInstanceId,
      publicationRevision: value.publicationRevision,
      projectName: value.projectName,
      exportTime: value.exportTime,
      eventGraphs,
      functionGraphs,
      charts,
      minds,
      docs,
      databases: value.databases,
    };
  } catch {
    throw new Error("Invalid project index response");
  }
}

export function isProjectDatabaseIndexRow(value: unknown): value is ProjectDatabaseIndexRow {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "id",
      "resourcePath",
      "revision",
      "engine",
      "schemaVersion",
      "required",
      "name",
    ])
  )
    return false;
  return (
    typeof value.id === "string" &&
    value.id.length > 0 &&
    typeof value.resourcePath === "string" &&
    value.resourcePath.length > 0 &&
    Number.isSafeInteger(value.revision) &&
    (value.revision as number) >= 0 &&
    isDatabaseEngine(value.engine) &&
    Number.isSafeInteger(value.schemaVersion) &&
    (value.schemaVersion as number) >= 0 &&
    typeof value.required === "boolean" &&
    (value.name === null || typeof value.name === "string")
  );
}

// ==================== 项目状态管理 API ====================

export type RevealProjectResourceRequest = {
  kind: import("@/shared/types/domain/resource").ResourceKind;
  resourceId: string;
};

export class ProjectService {
  // ==================== 项目级操作 ====================

  /** 获取当前后端项目 activation，供后创建的独立 WebView 建立 lifecycle identity。 */
  static async getProjectActivation(): Promise<ProjectActivationResult> {
    return parseProjectActivationResult(
      await invokeCommand<unknown>("get_current_project_activation"),
    );
  }

  /**
   * 按已捕获的项目索引版本读取 databases（含 schema）
   */
  static async getDatabases(projectInstanceId: string, expectedPublicationRevision: number) {
    return projectDatabasesSchema.parse(
      await invokeCommand<unknown>("get_project_databases", {
        projectInstanceId,
        expectedPublicationRevision,
      }),
    );
  }

  /**
   * 获取当前项目路径
   */
  static async getProjectPath(projectInstanceId: string): Promise<string | null> {
    return nullableProjectPathSchema.parse(
      await invokeCommand<unknown>("get_project_path", { projectInstanceId }),
    );
  }

  static async getProjectIndex(
    projectInstanceId: string,
    locale: string = DEFAULT_LANGUAGE,
    previous: Partial<Record<ProjectActivityPanelId, ActivityPanelSnapshot>> = {},
  ): Promise<ProjectIndexSnapshot> {
    let baselines = previous;
    for (let attempt = 0; attempt < 2; attempt++) {
      const value = await invokeCommand<unknown>("get_project_index", {
        projectInstanceId,
        locale,
        activityPanels: PROJECT_ACTIVITY_PANEL_IDS.map((panelId) => ({
          panelId,
          cursor: baselines[panelId]?.cursor ?? null,
        })),
      });
      if (!isRecord(value) || !hasExactKeys(value, ["index", "activityPanels"]))
        throw new Error("Invalid project index response");
      const index = parseProjectIndexRow(value.index);
      if (index.projectInstanceId !== projectInstanceId)
        throw new Error("Invalid project index response");
      const updates = value.activityPanels;
      if (isRecord(updates) && hasExactKeys(updates, PROJECT_ACTIVITY_PANEL_IDS)) {
        const parsed = PROJECT_ACTIVITY_PANEL_IDS.map((panelId) =>
          parseActivityPanelResponse(
            updates[panelId],
            panelId,
            projectInstanceId,
            baselines[panelId] ?? null,
          ),
        );
        if (
          parsed.every(
            (snapshot) =>
              snapshot && snapshot.document.publicationRevision === index.publicationRevision,
          )
        ) {
          return {
            index,
            activityPanels: Object.fromEntries(
              PROJECT_ACTIVITY_PANEL_IDS.map((panelId, i) => [panelId, parsed[i]!]),
            ) as ProjectIndexSnapshot["activityPanels"],
          };
        }
      }
      if (attempt === 0 && Object.keys(baselines).length > 0) {
        baselines = {};
        continue;
      }
      throw new IpcError({
        kind: "malformed",
        command: "get_project_index",
        code: "activity_panel_contract_invalid",
        details: null,
        incidentId: null,
        cause: null,
      });
    }
    throw new Error("Invalid project index response");
  }

  /**
   * 关闭指定项目并释放后端会话。
   */
  static async closeProject(projectInstanceId: string): Promise<void> {
    await invokeCommand("close_project", { projectInstanceId });
  }

  static async defaultProjectParentDirectory(): Promise<string> {
    return projectPathSchema.parse(
      await invokeCommand<unknown>("default_project_parent_directory"),
    );
  }

  static async validateNewProjectPath(path: string): Promise<void> {
    await invokeCommand("validate_new_project_path", { path });
  }

  static async createProject(
    name: string,
    path: string,
    operationId: string,
  ): Promise<LifecycleMutationResultDto> {
    return parseLifecycleMutationResult(
      await invokeCommand<unknown>("create_project", { name, path, operationId }),
    );
  }

  static async listRegisteredProjects(): Promise<ProjectRecordRow[]> {
    return parseProjectRecords(await invokeCommand<unknown>("list_registered_projects"));
  }

  static async cancelProjectPickerTask(): Promise<void> {
    await invokeCommand("cancel_project_picker_task");
  }

  static async cleanupInvalidRegisteredProjects(
    onProgress?: (event: ProjectCleanupProgressEvent) => void,
  ): Promise<CleanupInvalidProjectsResult> {
    const channel = projectProgressChannel(projectCleanupProgressSchema, onProgress);
    try {
      return projectCleanupResultSchema.parse(
        await invokeCommand<unknown>("cleanup_invalid_registered_projects", {
          onProgress: channel,
        }),
      );
    } finally {
      clearChannelMessageHandler(channel);
      untrackChannel(channel);
    }
  }

  static async scanProjectsInDirectory(
    directory: string,
    onProgress?: (event: ProjectScanProgressEvent) => void,
  ): Promise<ScanProjectsResult> {
    const channel = projectProgressChannel(projectScanProgressSchema, onProgress);
    try {
      return parseScanProjectsResult(
        await invokeCommand<unknown>("scan_projects_in_directory", {
          directory,
          onProgress: channel,
        }),
      );
    } finally {
      clearChannelMessageHandler(channel);
      untrackChannel(channel);
    }
  }

  static async registerProject(name: string, path: string): Promise<ProjectRecordRow> {
    return parseProjectRecord(await invokeCommand<unknown>("register_project", { name, path }));
  }

  static async removeRegisteredProject(id: string): Promise<void> {
    await invokeCommand("remove_registered_project", { id });
  }

  static async deleteRegisteredProjectFiles(
    id: string,
    expectedActiveProjectInstanceId: string | null,
    operationId: string,
  ): Promise<LifecycleMutationResultDto> {
    return parseLifecycleMutationResult(
      await invokeCommand<unknown>("delete_registered_project_files", {
        id,
        expectedActiveInstanceId: expectedActiveProjectInstanceId,
        operationId,
      }),
    );
  }

  static async toggleRegisteredProjectFavorite(id: string): Promise<boolean> {
    return projectFlagSchema.parse(
      await invokeCommand<unknown>("toggle_registered_project_favorite", { id }),
    );
  }

  /**
   * 从文件加载项目到状态管理器
   * 前端只传路径，后端负责加载；加载完成后会发出 ProjectLoaded 事件，前端通过 loadProject 刷新 store
   */
  static async loadProjectToState(path: string): Promise<ProjectActivationResult> {
    return parseProjectActivationResult(await invokeCommand<unknown>("load_project", { path }));
  }

  /**
   * 另存为：使用 Application 已选择的空目录，复制当前项目并切换工作路径。
   */
  static async saveProjectAs(
    projectInstanceId: string,
    operationId: string,
    path: string,
  ): Promise<LifecycleMutationResultDto> {
    await this.validateNewProjectPath(path);

    return parseLifecycleMutationResult(
      await invokeCommand<unknown>("save_project_as", {
        path,
        projectInstanceId,
        operationId,
      }),
    );
  }
  /** Execute one graph document and drain its streamed run eventGraphs. */
  static async executeGraph({
    projectInstanceId,
    graphPath,
    version,
    semanticInputHash,
    demand,
    onEvent,
  }: ExecuteGraphDocumentRequest): Promise<void> {
    const parsedDemand = parseExecutionDemandDto(demand);
    let observed: RunEvent["run"] | null = null;
    let terminal: RunEvent["kind"]["type"] | null = null;
    let consumerFailure: { error: unknown } | null = null;
    const deliver = (event: RunEvent) => {
      observed = event.run;
      if (["runCompleted", "runErrored", "runCancelled"].includes(event.kind.type))
        terminal = event.kind.type;
      try {
        onEvent?.(event);
      } catch (error) {
        consumerFailure = { error };
        throw error;
      }
    };
    const { channel, waitForStreamEnd } = bindExecutionEventChannel(deliver);
    try {
      try {
        await invokeCommand<void>("execute_graph", {
          projectInstanceId,
          graphPath,
          version,
          semanticInputHash,
          demand: parsedDemand,
          onEvent: channel,
        });
      } catch (error) {
        if (commandSentTerminalRunEvent(error)) {
          try {
            await waitForStreamEnd();
          } catch {
            // Backend classification remains authoritative.
          }
        }
        throw error;
      }
      await waitForStreamEnd();
    } catch (error) {
      if (consumerFailure && !commandSentTerminalRunEvent(error))
        throw (consumerFailure as { error: unknown }).error;
      const run = observed as RunEvent["run"] | null;
      if (run && !terminal) {
        try {
          const snapshot = await readExecutionSnapshot(projectInstanceId);
          const ended = snapshot.find(
            (event) =>
              event.run.executionSessionId === run.executionSessionId &&
              event.run.runId === run.runId &&
              ["runCompleted", "runErrored", "runCancelled"].includes(event.kind.type),
          );
          if (ended) deliver(ended);
        } catch {
          /* The caller keeps synchronization failure separate from run failure. */
        }
      }
      if (terminal === "runCompleted") return;
      throw error;
    } finally {
      untrackChannel(channel);
    }
  }

  static async cancelGraphRun(executionSessionId: string, runId: string): Promise<boolean> {
    return projectFlagSchema.parse(
      await invokeCommand<unknown>("cancel_graph_run", { executionSessionId, runId }),
    );
  }

  static async getProjectResourcePath(
    projectInstanceId: string,
    request: RevealProjectResourceRequest,
  ): Promise<string> {
    return projectPathSchema.parse(
      await invokeCommand<unknown>("get_project_resource_path", {
        projectInstanceId,
        kind: request.kind,
        resourceId: request.resourceId,
      }),
    );
  }
}
