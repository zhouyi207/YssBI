import {
  isLocalizedCatalogItemDto,
  type LocalizedCatalogItemDto,
  type LocalizedCategoryDto,
  type LocalizedParameterDto,
  type LocalizedPortDto,
} from '@/shared/types/domain/localizedCatalog';

export type LocalizedCatalogCategory = LocalizedCategoryDto;
export type LocalizedCatalogPort = LocalizedPortDto;
export type LocalizedCatalogParameter = LocalizedParameterDto;
export type LocalizedCatalogItem = LocalizedCatalogItemDto;
export const isLocalizedCatalogItem = isLocalizedCatalogItemDto;

/** Stable UI identity for a Catalog entry. Resource revisions are not identity. */
export function catalogItemKey(item: LocalizedCatalogItem): string {
  const descriptor = item.creation;
  if (descriptor.kind === 'resourceBound') {
    return `${descriptor.kind}:${descriptor.nodeTypeId}:${encodeURIComponent(descriptor.resourcePath)}`;
  }
  return `${descriptor.kind}:${descriptor.nodeTypeId}`;
}
