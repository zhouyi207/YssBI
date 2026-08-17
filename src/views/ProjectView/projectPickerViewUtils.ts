import type { ManagedProject } from '@/features/application/project';

export type ProjectSortMode = 'lastOpened' | 'name';

export function sortAndFilterProjects(
  items: ManagedProject[],
  query: string,
  mode: ProjectSortMode,
): ManagedProject[] {
  const normalizedQuery = query.trim().toLowerCase();
  const matchingItems = normalizedQuery
    ? items.filter((project) =>
      project.name.toLowerCase().includes(normalizedQuery)
      || project.path.toLowerCase().includes(normalizedQuery))
    : items;

  return [...matchingItems].sort((a, b) => {
    const favoriteDifference = Number(Boolean(b.isFavorite)) - Number(Boolean(a.isFavorite));
    if (favoriteDifference !== 0) return favoriteDifference;
    if (mode === 'name') return a.name.localeCompare(b.name, 'zh');
    return b.lastOpenedAt.localeCompare(a.lastOpenedAt);
  });
}

export function formatProjectStamp(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}
