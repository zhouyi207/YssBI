import type { NodeCatalogItem } from './types';

/** Filter catalog items by search query (title, nodeType, category). */
export function filterCatalogItems(items: NodeCatalogItem[], query: string): NodeCatalogItem[] {
  const trimmed = query.trim();
  if (!trimmed) return items;
  const q = trimmed.toLowerCase();
  return items.filter(
    (item) =>
      item.title.toLowerCase().includes(q) ||
      item.nodeType.toLowerCase().includes(q) ||
      item.category.some((c) => c.toLowerCase().includes(q)),
  );
}
