import { invoke } from '@tauri-apps/api/core';
import type { DataViewSourceValue, WindowDataPageResponse } from './types';

export class DataViewService {
  static async getPage(
    sourceId: string,
    offset: number,
    limit: number,
  ): Promise<WindowDataPageResponse> {
    return invoke<WindowDataPageResponse>('get_window_source_page', {
      key: sourceId,
      offset,
      limit,
    });
  }

  static async getValue(sourceId: string): Promise<DataViewSourceValue | null> {
    const json = await invoke<string | null>('get_window_source_value', { key: sourceId });
    return json ? (JSON.parse(json) as DataViewSourceValue) : null;
  }
}
