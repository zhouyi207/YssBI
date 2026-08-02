import { invoke } from '@tauri-apps/api/core';

export interface StaticNodeCreationDescriptorDto {
  kind: 'static';
  nodeTypeId: string;
}

export interface LocalizedCategoryDto {
  categoryId: string;
  title: string;
  searchText: string;
}

export interface LocalizedCatalogItemDto {
  nodeTypeId: string;
  title: string;
  description: string | null;
  documentation: string | null;
  categoryId: string;
  aliases: string[];
  technicalTerms: string[];
  pinyin?: string;
  creation: StaticNodeCreationDescriptorDto;
  searchText: string;
}

export interface LocalizedCatalogDto {
  projectInstanceId: string;
  registryFingerprint: string;
  resourcePublicationRevision: number;
  locale: string;
  categories: LocalizedCategoryDto[];
  items: LocalizedCatalogItemDto[];
}

export class CatalogService {
  static async getLocalizedCatalog(
    projectInstanceId: string,
    locale: string,
  ): Promise<LocalizedCatalogDto> {
    return invoke<LocalizedCatalogDto>('get_localized_node_catalog', {
      projectInstanceId,
      locale,
    });
  }
}
