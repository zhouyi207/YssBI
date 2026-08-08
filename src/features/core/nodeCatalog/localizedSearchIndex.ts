import {
  buildCatalogSearchDocument,
  matchesCatalogSearchDocument,
} from '@/features/domain/nodeCatalog/searchDocument';
import type { LocalizedCatalogResponse } from './nodeCatalogStore';

export interface LocalizedSearchIndex {
  readonly response: LocalizedCatalogResponse;
  search(query: string): LocalizedCatalogResponse['items'];
}

const indexes = new WeakMap<LocalizedCatalogResponse, LocalizedSearchIndex>();

export function getLocalizedSearchIndex(
  response: LocalizedCatalogResponse,
): LocalizedSearchIndex {
  const cached = indexes.get(response);
  if (cached) return cached;

  const documents = response.items.map((item) => ({
    item,
    document: buildCatalogSearchDocument(item),
  }));
  const index: LocalizedSearchIndex = {
    response,
    search: (query) => documents
      .filter(({ document }) => matchesCatalogSearchDocument(document, query))
      .map(({ item }) => item),
  };
  indexes.set(response, index);
  return index;
}
