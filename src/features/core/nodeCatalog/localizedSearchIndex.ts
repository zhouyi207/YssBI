import { searchLocalizedCatalogItems } from '@/features/domain/nodeCatalog/search';
import {
  catalogResponseKey,
  type LocalizedCatalogResponse,
} from './nodeCatalogStore';

export interface LocalizedSearchIndex {
  readonly response: LocalizedCatalogResponse;
  search(query: string): LocalizedCatalogResponse['items'];
}

const indexes = new Map<string, LocalizedSearchIndex>();

export function getLocalizedSearchIndex(
  response: LocalizedCatalogResponse,
): LocalizedSearchIndex {
  const key = catalogResponseKey(response);
  const cached = indexes.get(key);
  if (cached) return cached;

  const index: LocalizedSearchIndex = {
    response,
    search: (query) => searchLocalizedCatalogItems(response.items, query),
  };
  indexes.set(key, index);
  return index;
}
