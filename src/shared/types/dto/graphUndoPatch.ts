/**
 * 结构性 undo 补丁 — 对齐 Rust `schema::history::{GraphUndoPatch, NodeSubgraphDTO}`。
 * 前端仅透传至 `apply_graph_patch`；强类型防止误改字段与文档漂移。
 */

import type { DataType } from '../domain/dataType';
import type {
  PinDefinitionDTO,
  PinKind,
  PinRoleDTO,
  NodePosition,
} from '../domain/node';
import type { PinDirection } from '../domain/pin';
import type { NodeInstanceParamsDTO } from './nodeInstanceParams';

export interface ConnectionRebuildDTO {
  fromPin: string;
  toPin: string;
}

/** 对齐 Rust `TypeVarDefinition`（undo 子图 type_var_map 项） */
export interface TypeVarDefinitionDTO {
  id: string;
  constraints: readonly unknown[];
  bound?: DataType;
}

/** 动态 pin 精简契约（磁盘 / undo 中 `pinContract` 形态） */
export interface PinContractDTO {
  name: string;
  direction: PinDirection;
  kind: PinKind;
  role: PinRoleDTO;
  optional?: boolean;
}

/** 对齐 Rust `PinInstance` 磁盘序列化（`definition` 或 `pinContract` 二选一） */
export interface SubgraphPinInstanceDTO {
  id: string;
  nodeId?: string;
  definition?: PinDefinitionDTO;
  pinContract?: PinContractDTO;
  order?: number;
  typeVarId?: string;
  userValue?: unknown;
}

/** 对齐 Rust `NodeSubgraphDTO` */
export interface NodeSubgraphDTO {
  id: string;
  nodeType: string;
  position: NodePosition;
  /** 嵌套 tagged union（与 live `NodeInstanceDTO` flatten 不同） */
  instanceParams?: NodeInstanceParamsDTO;
  typeVarMap?: Record<string, TypeVarDefinitionDTO>;
  pins: SubgraphPinInstanceDTO[];
}

export interface GraphUndoPatch {
  nodes: NodeSubgraphDTO[];
  /** 删除 / 断连时冻结的动态邻居 */
  neighborNodes?: NodeSubgraphDTO[];
  connections: ConnectionRebuildDTO[];
}

export const EMPTY_GRAPH_UNDO_PATCH: GraphUndoPatch = {
  nodes: [],
  neighborNodes: [],
  connections: [],
};
