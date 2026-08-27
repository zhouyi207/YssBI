import { Channel } from "@tauri-apps/api/core";
import type {
    RunEvent,
    RunOutputChannelEvent,
} from "@/shared/types/dto/runEvent";
import type { ExecutionDemandDto } from "@/shared/types/dto/executionDemand";
import { parseInternalCompilationErrorDetails } from "@/shared/types/dto/executionError";
import { parseExecutionDemandDto } from "@/shared/types/dto/runEventParser";

import type {
  FunctionSignatureDto,
  HistoryStatusDto,
} from "@/shared/types/dto/editorMutation";
import type { FunctionEditorProjectionDto } from '@/shared/types/dto/editorProjection';
import type { DatabaseEngineDTO } from '@/shared/types/dto/database';
import type { WorksheetChartType } from '@/shared/types/domain/worksheet';
import {
  isFunctionEditorProjectionDto,
  isGraphResourcePath,
} from '@/shared/types/dto/editorProjectionGuards';
import type { CleanupInvalidProjectsResult, LifecycleMutationResultDto, ProjectRecordRow, ScanProjectsResult } from "@/shared/types/dto/project";
import {
  parseComputationSettingsMutationReceipt,
  parseComputationSettingsSnapshot,
  type ComputationSettingsMutationReceiptDto,
  type ComputationSettingsMutationRequestDto,
  type ComputationSettingsSnapshotDto,
} from '@/shared/types/dto/projectComputationSettings';

import { trackChannel, untrackChannel } from "@/services/devHmrIpc";
import { IpcError, invokeCommand, isIpcErrorCode } from '@/services/ipc';
import { bindExecutionEventChannel } from "./executionChannelDrain";

export type ProjectScanProgressEvent =
    | { kind: "scanning" }
    | { kind: "discovered"; count: number }
    | { kind: "registering"; current: number; total: number };

export type ProjectCleanupProgressEvent =
    | { kind: "checking"; current: number; total: number }
    | { kind: "removing"; removed: number; total: number };

export const PICKER_TASK_CANCELLED = "picker_task_cancelled";

export function isPickerTaskCancelledError(error: unknown): boolean {
    return isIpcErrorCode(error, PICKER_TASK_CANCELLED);
}

export function isExecutionCancelledError(error: unknown): boolean {
    return isIpcErrorCode(error, "run_cancelled");
}

function commandSentTerminalRunEvent(error: unknown): boolean {
    return error instanceof IpcError
        && error.details?.terminalRunEventSent === true;
}

interface ProjectGraphIndexRowBase {
    path: string;
    name: string;
    revision: number;
}

export interface ProjectEventGraphIndexRow extends ProjectGraphIndexRowBase {
    type: "event";
}

export interface ProjectFunctionGraphIndexRow extends ProjectGraphIndexRowBase {
    type: "function";
    functionRevision: number;
    functionSignature: FunctionSignatureDto;
    functionEditorProjection: FunctionEditorProjectionDto;
}

export type ProjectGraphIndexRow = ProjectEventGraphIndexRow | ProjectFunctionGraphIndexRow;

export interface ProjectWorksheetIndexRow {
  worksheetPath: string;
  name: string;
  databaseId: string;
  chartType: WorksheetChartType;
  revision: number;
}

export interface ProjectVariableIndexRow {
  id: string;
  resourcePath: string;
  revision: number;
  name: string;
  dataType: import('@/shared/types/domain').DataType;
  dataValue: import('@/shared/types/domain').DataValue;
  description: string;
  scope: import('@/shared/types/domain/variable').VariableScope;
  tags: string[];
  ownerGraphPath?: string | null;
  ownerGraphName?: string | null;
  ownerGraphKind?: 'event' | 'function' | null;
}

export interface ProjectDatabaseIndexRow {
  id: string;
  resourcePath: string;
  revision: number;
  engine: DatabaseEngineDTO;
  schemaVersion: number;
  required: boolean;
  name: string | null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length
    && keys.every((key) => Object.prototype.hasOwnProperty.call(value, key));
}

function isSafeRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}


function isFunctionSignature(value: unknown): value is FunctionSignatureDto {
  return isRecord(value)
    && !Array.isArray(value)
    && hasExactKeys(value, ['parameters', 'return_type'])
    && Array.isArray(value.parameters)
    && value.parameters.every((parameter) => isRecord(parameter)
      && !Array.isArray(parameter)
      && hasExactKeys(parameter, ['id', 'name', 'type_name'])
      && typeof parameter.id === 'string'
      && typeof parameter.name === 'string'
      && typeof parameter.type_name === 'string')
    && (value.return_type === null || typeof value.return_type === 'string');
}


export function parseProjectGraphIndexRow(value: unknown): ProjectGraphIndexRow {
  if (!isRecord(value) || Array.isArray(value)) {
    throw new Error('Invalid project graph index row');
  }
  const path = isGraphResourcePath(value.path) ? value.path : null;
  const common = path !== null
    && typeof value.name === 'string'
    && isSafeRevision(value.revision);
  if (value.type === 'event'
    && path?.startsWith('events/')
    && common
    && hasExactKeys(value, ['path', 'name', 'type', 'revision'])) {
    return value as unknown as ProjectEventGraphIndexRow;
  }
  if (value.type === 'function'
    && path?.startsWith('functions/')
    && common
    && hasExactKeys(value, [
      'path', 'name', 'type', 'revision', 'functionRevision', 'functionSignature',
      'functionEditorProjection',
    ])
    && isSafeRevision(value.functionRevision)
    && isFunctionSignature(value.functionSignature)
    && isFunctionEditorProjectionDto(value.functionEditorProjection)
    && value.functionEditorProjection.functionRevision === value.functionRevision) {
    return value as unknown as ProjectFunctionGraphIndexRow;
  }
  throw new Error('Invalid project graph index row');
}

function isWorksheetChartType(value: unknown): value is WorksheetChartType {
  return value === 'histogram' || value === 'scatter' || value === 'line';
}

function parseProjectWorksheetIndexRow(value: unknown): ProjectWorksheetIndexRow {
  if (!isRecord(value)
    || Array.isArray(value)
    || !hasExactKeys(value, ['worksheetPath', 'name', 'databaseId', 'chartType', 'revision'])
    || typeof value.worksheetPath !== 'string'
    || value.worksheetPath.length === 0
    || typeof value.name !== 'string'
    || value.name.trim().length === 0
    || typeof value.databaseId !== 'string'
    || !isWorksheetChartType(value.chartType)
    || !isSafeRevision(value.revision)) {
    throw new Error('Invalid project worksheet index row');
  }
  return value as unknown as ProjectWorksheetIndexRow;
}

function parseProjectIndexRow(value: unknown): ProjectIndexRow {
  if (!isRecord(value)
    || Array.isArray(value)
    || !hasExactKeys(value, [
      'projectInstanceId', 'publicationRevision', 'history', 'projectName',
      'exportTime', 'graphs', 'worksheets', 'variables', 'databases',
    ])
    || typeof value.projectInstanceId !== 'string'
    || !isSafeRevision(value.publicationRevision)
    || !isRecord(value.history)
    || Array.isArray(value.history)
    || !hasExactKeys(value.history, ['canUndo', 'canRedo'])
    || typeof value.history.canUndo !== 'boolean'
    || typeof value.history.canRedo !== 'boolean'
    || typeof value.projectName !== 'string'
    || typeof value.exportTime !== 'string'
    || !Array.isArray(value.graphs)
    || !Array.isArray(value.worksheets)
    || !Array.isArray(value.variables)
    || !Array.isArray(value.databases)
    || !value.databases.every(isProjectDatabaseIndexRow)) {
    throw new Error('Invalid project index response');
  }
  try {
    return {
      projectInstanceId: value.projectInstanceId,
      publicationRevision: value.publicationRevision,
      history: value.history as unknown as HistoryStatusDto,
      projectName: value.projectName,
      exportTime: value.exportTime,
      graphs: value.graphs.map(parseProjectGraphIndexRow),
      worksheets: value.worksheets.map(parseProjectWorksheetIndexRow),
      variables: value.variables as ProjectVariableIndexRow[],
      databases: value.databases,
    };
  } catch {
    throw new Error('Invalid project index response');
  }
}

function isSqlEngine(value: unknown): boolean {
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if (isRecord(value.sqlite)) {
    return hasExactKeys(value.sqlite, ['autoCreate']) && typeof value.sqlite.autoCreate === 'boolean';
  }
  if (isRecord(value.postgres)) {
    return hasExactKeys(value.postgres, ['ssl']) && typeof value.postgres.ssl === 'boolean';
  }
  return isRecord(value.mysql)
    && hasExactKeys(value.mysql, ['charset'])
    && typeof value.mysql.charset === 'string';
}

function isDatabaseEngine(value: unknown): value is DatabaseEngineDTO {
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if (isRecord(value.csv)) {
    return hasExactKeys(value.csv, ['path', 'delimiter', 'hasHeader', 'inferSchemaLength'])
      && typeof value.csv.path === 'string'
      && typeof value.csv.delimiter === 'string'
      && [...value.csv.delimiter].length === 1
      && typeof value.csv.hasHeader === 'boolean'
      && (value.csv.inferSchemaLength === null
        || (Number.isSafeInteger(value.csv.inferSchemaLength)
          && (value.csv.inferSchemaLength as number) >= 0));
  }
  if (isRecord(value.sql)) {
    return hasExactKeys(value.sql, ['engine', 'connectionString', 'table'])
      && isSqlEngine(value.sql.engine)
      && typeof value.sql.connectionString === 'string'
      && typeof value.sql.table === 'string';
  }
  if (isRecord(value.parquet)) {
    return hasExactKeys(value.parquet, ['path', 'columns'])
      && typeof value.parquet.path === 'string'
      && (value.parquet.columns === null
        || (Array.isArray(value.parquet.columns)
          && value.parquet.columns.every((column) => typeof column === 'string')));
  }
  if (isRecord(value.excel)) {
    return hasExactKeys(value.excel, ['path', 'sheet'])
      && typeof value.excel.path === 'string'
      && typeof value.excel.sheet === 'string';
  }
  if (isRecord(value.duckDb)) {
    return hasExactKeys(value.duckDb, ['path', 'table'])
      && typeof value.duckDb.path === 'string'
      && typeof value.duckDb.table === 'string';
  }
  return isRecord(value.inMemory)
    && hasExactKeys(value.inMemory, ['name'])
    && typeof value.inMemory.name === 'string';
}

export function isProjectDatabaseIndexRow(value: unknown): value is ProjectDatabaseIndexRow {
  if (!isRecord(value)
    || !hasExactKeys(value, [
      'id', 'resourcePath', 'revision', 'engine', 'schemaVersion', 'required', 'name',
    ])) return false;
  return typeof value.id === 'string'
    && value.id.length > 0
    && typeof value.resourcePath === 'string'
    && value.resourcePath.length > 0
    && Number.isSafeInteger(value.revision)
    && (value.revision as number) >= 0
    && isDatabaseEngine(value.engine)
    && Number.isSafeInteger(value.schemaVersion)
    && (value.schemaVersion as number) >= 0
    && typeof value.required === 'boolean'
    && (value.name === null || typeof value.name === 'string');
}

export interface ProjectActivationResult {
  path: string;
  projectInstanceId: string;
  activationRevision: number;
}

export interface ProjectIndexRow {
  projectInstanceId: string;
  projectName: string;
  exportTime: string;
  publicationRevision: number;
  history: HistoryStatusDto;
  graphs: ProjectGraphIndexRow[];
  worksheets: ProjectWorksheetIndexRow[];
  variables: ProjectVariableIndexRow[];
  databases: ProjectDatabaseIndexRow[];
}


// ==================== 项目状态管理 API ====================

export type RevealProjectResourceRequest = {
  kind: "graph" | "database" | "worksheet";
  resourceId: string;
};

export class ProjectService {
    // ==================== 项目级操作 ====================

    /** 获取当前后端项目 activation，供后创建的独立 WebView 建立 lifecycle identity。 */
    static async getProjectActivation(): Promise<ProjectActivationResult> {
        return await invokeCommand("get_current_project_activation");
    }

    /**
     * 分阶段加载第一步：获取 databases + variables（含 schema）
     */
    static async getDatabasesVariables(projectInstanceId: string): Promise<{
        databases: Record<string, unknown>;
        variables: Record<string, unknown>;
    }> {
        const data = await invokeCommand<{ databases: Record<string, unknown>; variables: Record<string, unknown> }>(
            "get_project_databases_variables",
            { projectInstanceId },
        );
        return { databases: data.databases || {}, variables: data.variables || {} };
    }

    /**
     * 获取当前项目路径
     */
    static async getProjectPath(projectInstanceId: string): Promise<string | null> {
        return await invokeCommand("get_project_path", { projectInstanceId });
    }

    static async getProjectIndex(projectInstanceId: string): Promise<ProjectIndexRow> {
        const value = await invokeCommand<unknown>("get_project_index", { projectInstanceId });
        return parseProjectIndexRow(value);
    }

    static async getProjectComputationSettings(
      projectInstanceId: string,
    ): Promise<ComputationSettingsSnapshotDto> {
      const value = await invokeCommand<unknown>('get_project_computation_settings', { projectInstanceId });
      return parseComputationSettingsSnapshot(value);
    }

    static async updateProjectComputationSettings(
      request: ComputationSettingsMutationRequestDto,
    ): Promise<ComputationSettingsMutationReceiptDto> {
      const value = await invokeCommand<unknown>('update_project_computation_settings', { request });
      return parseComputationSettingsMutationReceipt(value);
    }

    /**
     * 新建项目（清空当前状态）
     */
    static async newProject(): Promise<void> {
        await invokeCommand("new_project");
    }

    static async defaultProjectParentDirectory(): Promise<string> {
        return await invokeCommand("default_project_parent_directory");
    }

    static async validateNewProjectPath(path: string): Promise<void> {
        await invokeCommand("validate_new_project_path", { path });
    }

    static async createProject(
        name: string,
        path: string,
        operationId: string,
    ): Promise<LifecycleMutationResultDto> {
        return await invokeCommand("create_project", { name, path, operationId });
    }

    static async listRegisteredProjects(): Promise<ProjectRecordRow[]> {
        return await invokeCommand("list_registered_projects");
    }

    static async cancelProjectPickerTask(): Promise<void> {
        await invokeCommand("cancel_project_picker_task");
    }

    static async cleanupInvalidRegisteredProjects(
        onProgress?: (event: ProjectCleanupProgressEvent) => void,
    ): Promise<CleanupInvalidProjectsResult> {
        const channel = trackChannel(new Channel<ProjectCleanupProgressEvent>());
        channel.onmessage = (event) => {
            onProgress?.(event);
        };
        try {
            return await invokeCommand("cleanup_invalid_registered_projects", {
                onProgress: channel,
            });
        } finally {
            untrackChannel(channel);
        }
    }

    static async scanProjectsInDirectory(
        directory: string,
        onProgress?: (event: ProjectScanProgressEvent) => void,
    ): Promise<ScanProjectsResult> {
        const channel = trackChannel(new Channel<ProjectScanProgressEvent>());
        channel.onmessage = (event) => {
            onProgress?.(event);
        };
        try {
            return await invokeCommand("scan_projects_in_directory", {
                directory,
                onProgress: channel,
            });
        } finally {
            untrackChannel(channel);
        }
    }

    static async registerProject(name: string, path: string): Promise<ProjectRecordRow> {
        return await invokeCommand("register_project", { name, path });
    }

    static async removeRegisteredProject(id: string): Promise<void> {
        await invokeCommand("remove_registered_project", { id });
    }

    static async deleteRegisteredProjectFiles(
        id: string,
        expectedActiveProjectInstanceId: string | null,
        operationId: string,
    ): Promise<LifecycleMutationResultDto> {
        return await invokeCommand("delete_registered_project_files", {
            id,
            expectedActiveProjectInstanceId,
            operationId,
        });
    }

    static async toggleRegisteredProjectFavorite(id: string): Promise<boolean> {
        return await invokeCommand("toggle_registered_project_favorite", { id });
    }

    /**
     * 从文件加载项目到状态管理器
     * 前端只传路径，后端负责加载；加载完成后会发出 ProjectLoaded 事件，前端通过 loadProject 刷新 store
     */
    static async loadProjectToState(path: string): Promise<ProjectActivationResult> {
        return await invokeCommand("load_project", { path });
    }

    /**
     * Flush the current file-backed project to disk.
     */
    static async flushProject(
        projectInstanceId: string,
        operationId: string,
    ): Promise<import('@/shared/types/dto').ProjectSaveResultDto> {
        return await invokeCommand("flush_project", { projectInstanceId, operationId });
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

        return await invokeCommand<LifecycleMutationResultDto>("save_project_as", {
            path,
            projectInstanceId,
            operationId,
        });
    }
    /** Execute one graph document and drain its streamed run events. */
    static async executeGraphDocument(
        projectInstanceId: string,
        graphPath: string,
        demand: ExecutionDemandDto,
        onEvent?: (event: RunEvent) => void,
        onOutput?: (event: RunOutputChannelEvent) => void,
    ): Promise<void> {
        const parsedDemand = parseExecutionDemandDto(demand);
        const { channel, waitForStreamEnd } = bindExecutionEventChannel(onEvent, onOutput);
        try {
            try {
                await invokeCommand<void>('execute_graph_document', {
                    projectInstanceId,
                    graphPath,
                    demand: parsedDemand,
                    onEvent: channel,
                });
            } catch (error) {
                if (isIpcErrorCode(error, 'internal_compilation_failure')) {
                    parseInternalCompilationErrorDetails(error.details);
                    throw error;
                }
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
        } finally {
            untrackChannel(channel);
        }
    }

    static async cancelGraphRun(runId: string): Promise<boolean> {
        return invokeCommand<boolean>("cancel_graph_run", { runId });
    }


    static async getProjectResourcePath(
        projectInstanceId: string,
        request: RevealProjectResourceRequest,
    ): Promise<string> {
        return await invokeCommand<string>("get_project_resource_path", {
            projectInstanceId,
            kind: request.kind,
            resourceId: request.resourceId,
        });
    }

}
