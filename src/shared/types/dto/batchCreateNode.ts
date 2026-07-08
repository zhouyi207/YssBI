/**
 * 节点批量创建 DTO — 对齐 Rust `BatchCreateNodeRequest` / `batch_create_nodes` IPC。
 */

import {
  spawnParamsToInstanceParams,
  type NodeInstanceParamsDTO,
  type NodeSpawnParams,
} from './nodeInstanceParams';

export type {
  ParamsKind,
  NodeSpawnParams,
  NodeInstanceParamsDTO,
  CreateNodeSpawnParams,
} from './nodeInstanceParams';

export { spawnParamsToInstanceParams, NODE_INSTANCE_PARAMS_NONE } from './nodeInstanceParams';

/** 对齐 Rust `commands/command_graph/command_node.rs::BatchCreateNodeRequest` */
export interface BatchCreateNodeRequest {
  nodeType: string;
  x?: number;
  y?: number;
  params?: NodeSpawnParams;
}

/** `batch_create_nodes` invoke 单条 wire 形态 */
export interface BatchCreateNodeIpcItem {
  nodeType: string;
  x: number | null;
  y: number | null;
  params: NodeInstanceParamsDTO | null;
}

export function toBatchCreateNodeIpcItems(
  requests: readonly BatchCreateNodeRequest[],
): BatchCreateNodeIpcItem[] {
  return requests.map((request) => ({
    nodeType: request.nodeType,
    x: request.x ?? null,
    y: request.y ?? null,
    params: spawnParamsToInstanceParams(request.params),
  }));
}
