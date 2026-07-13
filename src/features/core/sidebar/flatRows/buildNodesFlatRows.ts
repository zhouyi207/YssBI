import {
  buildTreeFromItems,
  catalogItemKey,
  filterCatalogItems,
  flattenTree,
  type FlatRow,
  type NodeCatalogItem,
} from '@/features/domain/nodeCatalog';
import { nodeGroupKey, resolveGroupExpanded } from './groupExpandState';
import type { FlatSidebarRow } from './types';

export function buildNodesFlatRows(params: {
  items: NodeCatalogItem[];
  filterQuery: string;
  expandedGroups: Record<string, boolean>;
  noMatchesMessage: string;
}): FlatSidebarRow[] {
  const query = params.filterQuery.trim();
  if (params.items.length === 0 && !query) return [];

  const filtered = query ? filterCatalogItems(params.items, query) : params.items;
  if (filtered.length === 0) {
    return [
      {
        kind: 'empty',
        rowKey: 'empty:nodes-no-match',
        level: 0,
        message: params.noMatchesMessage,
      },
    ];
  }

  const { sortedChildren, allPaths } = buildTreeFromItems(filtered);
  const expandedPaths = new Set<string>();

  if (query) {
    allPaths.forEach((path) => expandedPaths.add(path));
  } else {
    allPaths.forEach((path) => {
      if (resolveGroupExpanded(params.expandedGroups, nodeGroupKey(path))) {
        expandedPaths.add(path);
      }
    });
  }

  const treeRows: FlatRow[] = [];
  flattenTree(sortedChildren, expandedPaths, '', 0, treeRows);

  const rows: FlatSidebarRow[] = [];
  for (const row of treeRows) {
    if (row.type === 'category') {
      rows.push({
        kind: 'group',
        rowKey: `group:${nodeGroupKey(row.path)}`,
        groupKey: nodeGroupKey(row.path),
        level: row.level,
        label: row.node.name,
        expanded: expandedPaths.has(row.path),
      });
      continue;
    }

    rows.push({
      kind: 'node',
      rowKey: `node:${catalogItemKey(row.item)}`,
      level: row.level,
      item: row.item,
    });
  }

  return rows;
}
