import { invoke } from '@tauri-apps/api/core';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import type { PlotColumnPairPayload } from '@/shared/types/domain/worksheet';

export class WorksheetService {
  static async createWorksheet(
    name?: string,
    databaseId?: string,
  ): Promise<WorksheetDocument> {
    return await invoke('create_worksheet', { name, databaseId });
  }

  static async loadWorksheet(worksheetId: string): Promise<WorksheetDocument> {
    return await invoke('load_worksheet', { worksheetId });
  }

  static async saveWorksheet(document: WorksheetDocument): Promise<void> {
    await invoke('save_worksheet', { document });
  }

  static async deleteWorksheet(worksheetId: string): Promise<void> {
    await invoke('delete_worksheet', { worksheetId });
  }

  static async getPlotColumnPair(
    databaseId: string,
    xCol: string,
    yCol: string,
    maxPoints?: number,
  ): Promise<PlotColumnPairPayload> {
    return await invoke('get_plot_column_pair', {
      databaseId,
      xCol,
      yCol,
      maxPoints,
    });
  }
}
