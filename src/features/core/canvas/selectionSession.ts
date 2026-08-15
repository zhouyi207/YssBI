export function unionSelectionIds(
  baseNodeIds: readonly string[],
  hitNodeIds: readonly string[],
): string[] {
  const selectedIds: string[] = [];
  const seen = new Set<string>();

  for (const id of [...baseNodeIds, ...hitNodeIds]) {
    if (seen.has(id)) continue;
    seen.add(id);
    selectedIds.push(id);
  }

  return selectedIds;
}
