import { invoke } from '@tauri-apps/api/core';
import {
  isLocalizedCatalogDto,
  type LocalizedCatalogDto,
} from '@/shared/types/dto/localizedCatalog';

export type {
  LocalizedCatalogDto,
  LocalizedCatalogItemDto,
  LocalizedCategoryDto,
  LocalizedParameterDto,
  LocalizedPortDto,
} from '@/shared/types/dto/localizedCatalog';

export class CatalogService {
  static async getLocalizedCatalog(
    projectInstanceId: string,
    locale: string,
  ): Promise<LocalizedCatalogDto> {
    const response: unknown = await invoke('get_localized_node_catalog', {
      projectInstanceId,
      locale,
    });
    if (!isLocalizedCatalogDto(response)) {
      throw new Error('Invalid localized node catalog response');
    }
    return response;
  }
}
