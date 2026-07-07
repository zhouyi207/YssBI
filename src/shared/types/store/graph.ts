/**
 * Store 层图数据结构
 * 用于 graphDataStore 的规范化存储格式
 * 与 DTO 兼容，便于从后端数据直接 hydrate
 */

import type { NodeId, PinId, GraphId, ConnectionId } from '../domain/ids';
export type { NodeId, PinId, GraphId, ConnectionId };
import type { GraphPosition, Graph } from '../domain/graph';
import type { PinDirection, PinType, PinUI } from '../domain/pin';
import type { DataType } from '../domain/dataType';
import type { GraphInstanceDTO } from '../dto/graph';

// ==================== NodeData ====================
/** 节点数据（Store 规范化格式，camelCase 与 DESIGN_RULE 一致） */
export interface NodeData {
  id: string;
  graphId: string;
  nodeType: string;
  category: string[];
  title: string;
  inputs: string[];   // Pin IDs
  outputs: string[]; // Pin IDs
  uiStyle: string;
  description?: string;
  position: { x: number; y: number };
  /** 以下为 UI 扩展字段 */
  isInternal?: boolean;
  /** 参数类型判别 */
  paramsKind?: 'none' | 'variable' | 'subGraph' | 'dataFrame';
  variableId?: string;
  variableName?: string;
  variableType?: string;
  subGraphId?: string;
  dataframeId?: string;
}

// ==================== PinData ====================
/** Pin 实体数据。连接关系不在这里保存；运行时连接状态从 pinConnections 派生。 */
export interface PinData {
  id: string;
  nodeId: string;
  name: string;
  type: PinType | string;
  direction: PinDirection;
  defaultValue?: unknown;
  userValue?: unknown;
  containerType?: string;
  typeDisplay?: string;
  dataType?: DataType;
  optional?: boolean;
  ui?: PinUI;
}

/** UI 运行时 Pin 视图，连接状态从 pinConnections 派生。 */
export type PinView = PinData & {
  connected: boolean;
  linkCount: number;
  connectionIds: string[];
};

// ==================== ConnectionData ====================
/** 连接数据（Store 格式，含派生 id） */
export interface ConnectionData {
  id: string;   // 格式: "fromPinId->toPinId"
  from: string; // PinId
  to: string;   // PinId
}

// ==================== GraphData ====================
/** 图完整数据（getGraphById 返回格式） */
export interface GraphData {
  id: string;
  name: string;
  type: 'event' | 'function';
  functionInputs?: import('../domain/graph').FunctionSignaturePin[];
  functionOutputs?: import('../domain/graph').FunctionSignaturePin[];
  nodes: NodeData[];
  pins: PinData[];
  connections: ConnectionData[] | { connections: Array<{ fromPin: string; toPin: string }> };
  canvas: GraphPosition;
}

/** 运行时节点（inputs/outputs 可为 Pin ID 或完整 Pin 对象） */
export interface RuntimeNodeInput {
  id: string;
  graphId?: string;
  nodeType: string;
  category?: string[];
  title?: string;
  position?: { x: number; y: number };
  inputs?: (string | PinData | PinView)[];
  outputs?: (string | PinData | PinView)[];
  uiStyle?: string;
  description?: string;
  isInternal?: boolean;
  paramsKind?: 'none' | 'variable' | 'subGraph' | 'dataFrame';
  variableId?: string;
  variableName?: string;
  variableType?: string;
  subGraphId?: string;
  dataframeId?: string;
}

/** 图数据输入（addGraphFromData 等接受 nodes 含 Pin 对象的格式） */
export interface GraphDataInput extends Omit<GraphData, 'nodes'> {
  nodes: RuntimeNodeInput[];
}

/** addGraphFromData 接受的图格式（GraphData、GraphDataInput、domain Graph 或后端 DTO） */
export type GraphDataLike = GraphData | GraphDataInput | Graph | GraphInstanceDTO;
