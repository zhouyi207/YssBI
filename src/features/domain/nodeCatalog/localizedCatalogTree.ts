import type {
  LocalizedCatalogItemDto,
  LocalizedCategoryDto,
} from '@/shared/types/dto/localizedCatalog';

export interface LocalizedCatalogTreeNode {
  category: LocalizedCategoryDto;
  children: LocalizedCatalogTreeNode[];
  items: LocalizedCatalogItemDto[];
}

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
    || left.nodeTypeId.localeCompare(right.nodeTypeId);
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
