import type { LocalizedCatalogItem } from './catalogItem';

function normalizeQuery(value: string): string {
  return value
    .normalize('NFKD')
    .replace(/\p{Mark}/gu, '')
    .toLowerCase()
    .replace(/[^\p{Letter}\p{Number}]+/gu, ' ')
    .trim();
}

function searchableText(item: LocalizedCatalogItem): string {
  return normalizeQuery(
    [item.title, ...item.aliases].join(' '),
  );
}

export function searchLocalizedCatalogItems(
  items: readonly LocalizedCatalogItem[],
  query: string,
): LocalizedCatalogItem[] {
  const terms = normalizeQuery(query).split(' ').filter(Boolean);
  if (terms.length === 0) return [...items];

  return items.filter((item) => {
    const text = searchableText(item);
    return terms.every((term) => text.includes(term));
  });
}
