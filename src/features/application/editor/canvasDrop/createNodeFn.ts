import type { NodeSpawnParams } from '@/shared/types/dto/nodeInstanceParams';

export type CreateNodeFn = (
  nodeType: string,
  position: { x: number; y: number },
  params?: NodeSpawnParams,
) => Promise<{ nodeId: string; pinIds: string[] } | undefined>;
