import type { NodeCatalogItem } from './types';

export const NODE_CATALOG_ROW_HEIGHT = 30;

export interface TreeCategory {
  name: string;
  isLeaf: false;
  children: Record<string, TreeNode>;
}

export interface TreeLeaf {
  name: string;
  isLeaf: true;
  item: NodeCatalogItem;
}

export type TreeNode = TreeCategory | TreeLeaf;

export type FlatRow =
  | { type: 'category'; level: number; node: TreeCategory; path: string }
  | { type: 'leaf'; level: number; item: NodeCatalogItem };

const TREE_SORT = (a: TreeNode, b: TreeNode) => {
  if (a.isLeaf !== b.isLeaf) return a.isLeaf ? 1 : -1;
  return a.name.localeCompare(b.name);
};

export function buildTreeFromItems(items: NodeCatalogItem[]): {
  tree: TreeCategory;
  allPaths: Set<string>;
  sortedChildren: TreeNode[];
} {
  const tree: TreeCategory = { name: 'Root', isLeaf: false, children: {} };
  const allPaths = new Set<string>();

  items.forEach((item) => {
    let current = tree;
    let path = '';
    item.category.forEach((cat) => {
      path = path ? `${path}/${cat}` : cat;
      allPaths.add(path);
      if (!current.children[cat]) {
        current.children[cat] = { name: cat, isLeaf: false, children: {} };
      }
      current = current.children[cat] as TreeCategory;
    });
    const leafKey = `${item.nodeType}-${item.overrides?.variableId ?? item.overrides?.subGraphPath ?? ''}`;
    current.children[leafKey] = { name: item.title, isLeaf: true, item };
  });

  const sortedChildren = Object.values(tree.children).sort(TREE_SORT);
  return { tree, allPaths, sortedChildren };
}

export function flattenTree(
  children: TreeNode[],
  expandedPaths: Set<string>,
  parentPath: string,
  level: number,
  out: FlatRow[],
) {
  const sorted = [...children].sort(TREE_SORT);
  for (const child of sorted) {
    if (child.isLeaf) {
      out.push({ type: 'leaf', level, item: child.item });
    } else {
      const path = parentPath ? `${parentPath}/${child.name}` : child.name;
      out.push({ type: 'category', level, node: child, path });
      if (expandedPaths.has(path)) {
        flattenTree(Object.values(child.children), expandedPaths, path, level + 1, out);
      }
    }
  }
}
