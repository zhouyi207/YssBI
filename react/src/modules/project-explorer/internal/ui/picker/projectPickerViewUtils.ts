import type { ManagedProject } from "@/features/application/project";

export type ProjectSortMode = "lastOpened" | "name";

export function sortAndFilterProjects(
  items: ManagedProject[],
  query: string,
  mode: ProjectSortMode,
): ManagedProject[] {
  const normalizedQuery = query.trim().toLowerCase();
  const matchingItems = normalizedQuery
    ? items.filter(
        (project) =>
          project.name.toLowerCase().includes(normalizedQuery) ||
          project.path.toLowerCase().includes(normalizedQuery),
      )
    : items;

  return [...matchingItems].sort((a, b) => {
    const favoriteDifference = Number(Boolean(b.isFavorite)) - Number(Boolean(a.isFavorite));
    if (favoriteDifference !== 0) return favoriteDifference;
    if (mode === "name") return a.name.localeCompare(b.name, "zh");
    return b.lastOpenedAt.localeCompare(a.lastOpenedAt);
  });
}

export function formatProjectStamp(value: string): string {
  const calendar = /^(\d{4}-\d{2}-\d{2})[T ](\d{2}:\d{2})/.exec(value);
  if (calendar) return `${calendar[1]} ${calendar[2]}`;
  // Registry stamps are Unix seconds. Calendar strings above already own their clock fields.
  if (!/^\d+$/.test(value)) return value;
  const date = new Date(Number(value) * 1000);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}
