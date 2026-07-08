import type { NodeSpawnParams } from '@/shared/types/dto/nodeInstanceParams';

export type CanvasCreateNodeParams = NodeSpawnParams;

export type CreateNodeFn = (
  nodeType: string,
  position: { x: number; y: number },
  params?: CanvasCreateNodeParams,
) => Promise<{ nodeId: string; pinIds: string[] } | undefined>;
