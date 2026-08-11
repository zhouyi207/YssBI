import { invoke } from '@tauri-apps/api/core';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import type { PlotColumnPairPayload } from '@/shared/types/domain/worksheet';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import { parseResourceMutationResultDto } from '@/shared/types/dto/resourceMutationResultWireParser';


export class WorksheetService {
  static async createWorksheet(
    projectInstanceId: string,
    operationId: string,
    name: string,
    databaseId?: string,
  ): Promise<ResourceMutationResultDto> {
    return parseResourceMutationResultDto(await invoke<unknown>('create_worksheet', {
      projectInstanceId,
      operationId,
      name,
      databaseId,
    }));
  }

  static async duplicateWorksheet(
    projectInstanceId: string,
    operationId: string,
    worksheetPath: string,
    expectedRevision: number,
  ): Promise<ResourceMutationResultDto> {
    return parseResourceMutationResultDto(await invoke<unknown>('duplicate_worksheet', {
      projectInstanceId,
      operationId,
      worksheetPath,
      expectedRevision,
    }));
  }

  static async loadWorksheet(
    projectInstanceId: string,
    worksheetPath: string,
  ): Promise<WorksheetDocument> {
    return await invoke('load_worksheet', { projectInstanceId, worksheetPath });
  }

  static async saveWorksheet(
    projectInstanceId: string,
    operationId: string,
    worksheetPath: string,
    expectedRevision: number,
    document: WorksheetDocument,
  ): Promise<ResourceMutationResultDto> {
    return parseResourceMutationResultDto(await invoke<unknown>('save_worksheet', {
      projectInstanceId,
      operationId,
      worksheetPath,
      expectedRevision,
      document,
    }));
  }

  static async renameWorksheet(
    projectInstanceId: string,
    operationId: string,
    worksheetPath: string,
    expectedRevision: number,
    newName: string,
    lifecycleToken: number,
  ): Promise<ResourceMutationResultDto> {
    return parseResourceMutationResultDto(await invoke<unknown>('rename_worksheet_resource', {
      projectInstanceId,
      operationId,
      worksheetPath,
      expectedRevision,
      newName,
      lifecycleToken,
    }));
  }

  static async removeWorksheet(
    projectInstanceId: string,
    operationId: string,
    worksheetPath: string,
    expectedRevision: number,
  ): Promise<ResourceMutationResultDto> {
    return parseResourceMutationResultDto(await invoke<unknown>('remove_worksheet', {
      projectInstanceId,
      operationId,
      worksheetPath,
      expectedRevision,
    }));
  }

  static async getPlotColumnPair(
    projectInstanceId: string,
    databaseId: string,
    xCol: string,
    yCol: string,
    maxPoints?: number,
  ): Promise<PlotColumnPairPayload> {
    return await invoke('get_plot_column_pair', {
      projectInstanceId,
      databaseId,
      xCol,
      yCol,
      maxPoints,
    });
  }
}
