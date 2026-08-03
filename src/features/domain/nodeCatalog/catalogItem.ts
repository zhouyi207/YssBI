import {
  isLocalizedCatalogItemDto,
  type LocalizedCatalogItemDto,
  type LocalizedCategoryDto,
  type LocalizedParameterDto,
  type LocalizedPortDto,
} from '@/shared/types/dto/localizedCatalog';

export type LocalizedCatalogCategory = LocalizedCategoryDto;
export type LocalizedCatalogPort = LocalizedPortDto;
export type LocalizedCatalogParameter = LocalizedParameterDto;
export type LocalizedCatalogItem = LocalizedCatalogItemDto;
export const isLocalizedCatalogItem = isLocalizedCatalogItemDto;
