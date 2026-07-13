export const NODE_GROUP_PREFIX = 'nodes:' as const;

export function nodeGroupKey(categoryPath: string): string {
  return `${NODE_GROUP_PREFIX}${categoryPath}`;
}

/** Default expanded when absent from persisted storage. */
export function resolveGroupExpanded(
  expandedGroups: Record<string, boolean>,
  groupKey: string,
): boolean {
  if (groupKey in expandedGroups) return expandedGroups[groupKey];
  return true;
}
