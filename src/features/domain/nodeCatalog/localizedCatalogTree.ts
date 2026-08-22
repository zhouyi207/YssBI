import type {
  LocalizedCatalogItemDto,
  LocalizedCategoryDto,
} from '@/shared/types/dto/localizedCatalog';
import { catalogItemKey } from './catalogItem';

export interface LocalizedCatalogTreeNode {
  category: LocalizedCategoryDto;
  children: LocalizedCatalogTreeNode[];
  items: LocalizedCatalogItemDto[];
}

export type LocalizedCatalogBrowserRow =
  | {
      kind: 'category';
      rowKey: string;
      category: LocalizedCategoryDto;
      depth: number;
    }
  | {
      kind: 'item';
      rowKey: string;
      item: LocalizedCatalogItemDto;
      depth: number;
    };

function compareCategories(
  left: LocalizedCatalogTreeNode,
  right: LocalizedCatalogTreeNode,
): number {
  return left.category.order - right.category.order
    || left.category.title.localeCompare(right.category.title)
    || left.category.categoryId.localeCompare(right.category.categoryId);
}

function compareItems(
  left: LocalizedCatalogItemDto,
  right: LocalizedCatalogItemDto,
): number {
  return left.title.localeCompare(right.title)
    || left.nodeTypeId.localeCompare(right.nodeTypeId)
    || catalogItemKey(left).localeCompare(catalogItemKey(right));
}

export function buildLocalizedCatalogTree(
  categories: readonly LocalizedCategoryDto[],
  items: readonly LocalizedCatalogItemDto[],
): LocalizedCatalogTreeNode[] {
  const nodes = new Map<string, LocalizedCatalogTreeNode>(
    categories.map((category) => [category.categoryId, {
      category,
      children: [],
      items: [],
    }]),
  );

  for (const item of items) {
    nodes.get(item.categoryId)?.items.push(item);
  }

  const roots: LocalizedCatalogTreeNode[] = [];
  for (const node of nodes.values()) {
    const parentId = node.category.parentCategoryId;
    const parent = parentId ? nodes.get(parentId) : undefined;
    if (!parent || parent === node) {
      roots.push(node);
    } else {
      parent.children.push(node);
    }
  }

  const retained = new Set<LocalizedCatalogTreeNode>();
  const retainPopulated = (
    node: LocalizedCatalogTreeNode,
    visiting: Set<LocalizedCatalogTreeNode>,
  ): boolean => {
    if (visiting.has(node)) return false;
    visiting.add(node);
    node.children = node.children.filter((child) => retainPopulated(child, visiting));
    visiting.delete(node);
    node.children.sort(compareCategories);
    node.items.sort(compareItems);
    const populated = node.items.length > 0 || node.children.length > 0;
    if (populated) retained.add(node);
    return populated;
  };

  const populatedRoots = roots.filter((root) => retainPopulated(root, new Set()));
  for (const node of nodes.values()) {
    if (!retained.has(node) && retainPopulated(node, new Set())) {
      populatedRoots.push(node);
    }
  }
  return [...new Set(populatedRoots)].sort(compareCategories);
}

export function collectLocalizedCatalogCategoryIds(
  tree: readonly LocalizedCatalogTreeNode[],
): Set<string> {
  const categoryIds = new Set<string>();
  const visit = (node: LocalizedCatalogTreeNode) => {
    categoryIds.add(node.category.categoryId);
    node.children.forEach(visit);
  };
  tree.forEach(visit);
  return categoryIds;
}

export function flattenLocalizedCatalogTree(
  tree: readonly LocalizedCatalogTreeNode[],
  expandedCategoryIds: ReadonlySet<string>,
): LocalizedCatalogBrowserRow[] {
  const rows: LocalizedCatalogBrowserRow[] = [];

  const visit = (node: LocalizedCatalogTreeNode, depth: number) => {
    rows.push({
      kind: 'category',
      rowKey: `category:${node.category.categoryId}`,
      category: node.category,
      depth,
    });
    if (!expandedCategoryIds.has(node.category.categoryId)) return;

    node.items.forEach((item) => {
      rows.push({
        kind: 'item',
        rowKey: `item:${catalogItemKey(item)}`,
        item,
        depth: depth + 1,
      });
    });
    node.children.forEach((child) => visit(child, depth + 1));
  };

  tree.forEach((node) => visit(node, 0));
  return rows;
}
