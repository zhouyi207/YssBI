import type { LocalizedCatalogItem } from "./catalogItem";
import { buildCatalogSearchDocument, matchesCatalogSearchDocument } from "./searchDocument";

export function searchLocalizedCatalogItems(
  items: readonly LocalizedCatalogItem[],
  query: string,
): LocalizedCatalogItem[] {
  return items.filter((item) =>
    matchesCatalogSearchDocument(buildCatalogSearchDocument(item), query),
  );
}
