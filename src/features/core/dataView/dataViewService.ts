import { invoke } from '@tauri-apps/api/core';
import type { SourceDescriptor, SourcePage, SourceValue } from './types';

export class SourceService {
  static async getDescriptor(sourceId: string): Promise<SourceDescriptor | null> {
    return invoke<SourceDescriptor | null>('get_result_source_descriptor', { sourceId });
  }

  static async getPinDescriptor(graphId: string, pinId: string): Promise<SourceDescriptor | null> {
    return invoke<SourceDescriptor | null>('get_pin_result_descriptor', { graphId, pinId });
  }

  static async getPage(
    sourceId: string,
    offset: number,
    limit: number,
  ): Promise<SourcePage> {
    return invoke<SourcePage>('get_result_source_page', {
      sourceId,
      offset,
      limit,
    });
  }

  static async getValue(sourceId: string): Promise<SourceValue | null> {
    return invoke<SourceValue | null>('get_result_source_value', { sourceId });
  }

  static async releaseResultSource(sourceId: string): Promise<void> {
    await invoke('release_result_source', { sourceId });
  }
}
