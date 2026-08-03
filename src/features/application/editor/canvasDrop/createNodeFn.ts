import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';

export type CreateNodeFn = (
  descriptor: NodeCreationDescriptor,
  position: { x: number; y: number },
) => Promise<boolean>;
