import type { LocalizedCatalogItem } from "@/features/domain/nodeCatalog/catalogItem";
import type { ResourceBoundCreateArgsDto } from "@/shared/types/domain/nodeCreationDescriptor";
import type { NodeSpawnTemplate } from "@/features/core/dnd";
import { findResourceCatalogItem } from "@/features/core/nodeCatalog/resourceCatalogIndex";
import type { CreateNodeFn } from "./createNodeFn";

export interface SpawnFromTemplateContext {
  createNode: CreateNodeFn;
}

export function findResourceNodeSpawnTemplate(
  items: readonly LocalizedCatalogItem[],
  resourcePath: string,
  createArgsKind: ResourceBoundCreateArgsDto["kind"],
  nodeTypeId?: string,
): NodeSpawnTemplate | null {
  const item = findResourceCatalogItem(items, resourcePath, createArgsKind, nodeTypeId);
  return item ? { title: item.title, descriptor: item.creation } : null;
}

export async function spawnNodeFromTemplate(
  template: NodeSpawnTemplate,
  worldPosition: { x: number; y: number },
  ctx: SpawnFromTemplateContext,
): Promise<boolean> {
  return ctx.createNode(template.descriptor, worldPosition);
}
