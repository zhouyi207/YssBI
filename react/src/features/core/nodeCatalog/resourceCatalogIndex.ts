import type { LocalizedCatalogItem } from "@/features/domain/nodeCatalog/catalogItem";
import type { ResourceBoundCreateArgsDto } from "@/shared/types/domain/nodeCreationDescriptor";

// Indexes share the lifetime of their immutable published item arrays.
const indexes = new WeakMap<
  readonly LocalizedCatalogItem[],
  ReadonlyMap<string, readonly LocalizedCatalogItem[]>
>();

export function findResourceCatalogItem(
  items: readonly LocalizedCatalogItem[],
  resourcePath: string,
  createArgsKind: ResourceBoundCreateArgsDto["kind"],
  nodeTypeId?: string,
): LocalizedCatalogItem | null {
  let index = indexes.get(items);
  if (!index) {
    const byPath = new Map<string, LocalizedCatalogItem[]>();
    for (const item of items) {
      if (item.resourcePath == null) continue;
      const candidates = byPath.get(item.resourcePath);
      if (candidates) candidates.push(item);
      else byPath.set(item.resourcePath, [item]);
    }
    index = byPath;
    indexes.set(items, index);
  }
  return (
    index
      .get(resourcePath)
      ?.find(
        (candidate) =>
          candidate.available &&
          candidate.creation.kind === "resourceBound" &&
          candidate.creation.resourcePath === resourcePath &&
          candidate.creation.createArgs.kind === createArgsKind &&
          (nodeTypeId === undefined || candidate.creation.nodeTypeId === nodeTypeId),
      ) ?? null
  );
}
