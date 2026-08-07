import { invoke } from '@tauri-apps/api/core';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import type { PlotColumnPairPayload } from '@/shared/types/domain/worksheet';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';

export interface WorksheetMutationResultDto {
  operationId: string;
  result: ResourceMutationResultDto;
  document: WorksheetDocument;
}

export class WorksheetService {
  static async createWorksheet(
    projectInstanceId: string,
    operationId: string,
    name?: string,
    databaseId?: string,
  ): Promise<WorksheetMutationResultDto> {
    return await invoke<WorksheetMutationResultDto>('create_worksheet', {
      projectInstanceId,
      operationId,
      name,
      databaseId,
    });
  }

  static async loadWorksheet(
    projectInstanceId: string,
    worksheetId: string,
  ): Promise<WorksheetDocument> {
    return await invoke('load_worksheet', { projectInstanceId, worksheetId });
  }

  static async saveWorksheet(
    projectInstanceId: string,
    operationId: string,
    document: WorksheetDocument,
  ): Promise<WorksheetMutationResultDto> {
    return await invoke<WorksheetMutationResultDto>('save_worksheet', {
      projectInstanceId,
      operationId,
      document,
    });
  }

  static async deleteWorksheet(
    projectInstanceId: string,
    operationId: string,
    worksheetId: string,
  ): Promise<WorksheetMutationResultDto> {
    return await invoke<WorksheetMutationResultDto>('delete_worksheet', {
      projectInstanceId,
      operationId,
      worksheetId,
    });
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
