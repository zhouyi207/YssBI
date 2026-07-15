/**
 * Store 层图数据结构
 *
 * ## Hydrate 契约（权威类型：`GraphData`）
 *
 * `graphDataStore` 的实体桶（`GraphEntityBucket`）只接受经 `normalizeGraphDataLike` 规范化后的形态。
 * 入站 API：`hydrateGraphs` / `addGraphFromData(graphPath, graph: GraphDataLike)`。
 *
 * ### `GraphDataLike`（入站联合）
 *
 * | 分支 | 来源 | 说明 |
 * | --- | --- | --- |
 * | `GraphData` | Store 快照 / 已规范化 | 直接写入，`nodes[].inputs/outputs` 为 PinId[] |
 * | `GraphDataInput` | 编辑器批量替换 | `nodes` 为 `RuntimeNodeInput[]` |
 * | `Graph` | domain / 项目导出 | `connections` 为 `{ connections: ConnectionItem[] }`；节点 pin 为完整对象 |
 * | `GraphInstanceDTO` | IPC `get_graph` 等 | camelCase；`connections` 多种历史形态 |
 *
 * ### `RuntimeNodeInput`（运行时节点入站）
 *
 * 用于 `GraphDataInput.nodes`。与 store `NodeData` 的差异：
 *
 * - `inputs` / `outputs`：**PinId 字符串** 或 **完整 `PinData` / `PinView` 对象**（hydrate 时统一为 PinId[]）
 * - `category` / `title` / `position` 等可选；缺失时 normalize 填默认值
 * - 连接关系**不**写在 pin 的废弃 `links` 字段；以图级 `connections` + `pinConnections` 为唯一真源
 *
 * ### 规范化规则
 *
 * 1. **结构**（`normalizeGraphDataLike` — `dto/graphModel.ts`）：节点 pin 引用、pin DTO、connections
 * 2. **展示 enrich**（`graphDataStore.buildGraphBucket`）：title / category 从节点注册表推导；`uiStyle` 仅在 `toUiNode` 视图层推导
 *
 * ### 出站（导出 / IPC 前）
 *
 * - Store → domain：`graphDataToDomainGraph`（节点嵌入 pin 对象，connections 包装）
 * - DTO 窄化：`graphInstanceDtoToGraphData` → `normalizeGraphDataLike`
 *
 * ### 测试夹具
 *
 * 新建图 store / sync / canvas 测试优先 `@/tests/helpers/graphFixtures` 的 `makeTestGraph()`，
 * 避免手写 `{ nodes: [...] }` 与 hydrate 规则漂移。
 *
 * 设计说明：[docs/adr/graph-store-hydrate.md](../../../docs/adr/graph-store-hydrate.md)
 */

import type { NodeId, PinId, GraphPath, ConnectionId } from '../domain/ids';
export type { NodeId, PinId, GraphPath, ConnectionId };
import type { Graph } from '../domain/graph';
import type { PinDirection, PinUI, RuntimePinKind } from '../domain/pin';
import type { DataType } from '../domain/dataType';
import type { GraphInstanceDTO } from '../dto/graph';

import type { ParamsKind } from '../dto/nodeInstanceParams';

// ==================== NodeData ====================
/** 节点数据（Store 规范化格式，camelCase 与 DESIGN_RULE 一致） */
export interface NodeData {
  id: string;
  graphPath: string;
  nodeType: string;
  category: string[];
  title: string;
  inputs: string[];   // Pin IDs
  outputs: string[]; // Pin IDs
  description?: string;
  position: { x: number; y: number };
  /** 以下为 UI 扩展字段 */
  isInternal?: boolean;
  /** 参数类型判别（扁平字段，见 `NodeInstanceParamsDTO` tagged union） */
  paramsKind?: ParamsKind;
  variableId?: string;
  variableName?: string;
  variableType?: string;
  subGraphPath?: string;
  dataframeId?: string;
}

// ==================== PinData ====================
/** Pin 实体数据。连接关系不在这里保存；运行时连接状态从 pinConnections 派生。 */
export interface PinData {
  id: string;
  nodeId: string;
  name: string;
  type: RuntimePinKind;
  direction: PinDirection;
  defaultValue?: unknown;
  userValue?: unknown;
  dataType?: DataType;
  optional?: boolean;
  ui?: PinUI;
  validationWarning?: string;
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
/** 图完整数据（store 权威格式；`connections` 为 `ConnectionData[]`，持久化经 `graphDataToDomainGraph` 包装） */
export interface GraphData {
  /** 图资源相对路径（与 Domain `Graph.path`、store 桶键一致） */
  path: string;
  name: string;
  type: 'event' | 'function';
  functionInputs?: import('../domain/graph').FunctionSignaturePin[];
  functionOutputs?: import('../domain/graph').FunctionSignaturePin[];
  nodes: NodeData[];
  pins: PinData[];
  connections: ConnectionData[];
}

/**
 * 运行时节点入站形态。
 * @see 模块顶部的 Hydrate 契约 — `inputs`/`outputs` 可为 PinId 或完整 Pin 对象。
 */
export interface RuntimeNodeInput {
  id: string;
  graphPath?: string;
  nodeType: string;
  category?: string[];
  title?: string;
  position?: { x: number; y: number };
  inputs?: (string | PinData | PinView)[];
  outputs?: (string | PinData | PinView)[];
  description?: string;
  isInternal?: boolean;
  paramsKind?: ParamsKind;
  variableId?: string;
  variableName?: string;
  variableType?: string;
  subGraphPath?: string;
  dataframeId?: string;
}

/** 图数据输入：`nodes` 为 `RuntimeNodeInput[]`，其余字段同 `GraphData` */
export interface GraphDataInput extends Omit<GraphData, 'nodes'> {
  nodes: RuntimeNodeInput[];
}

/**
 * `addGraphFromData` / `hydrateGraphs` 入站联合类型。
 * 一律经 `normalizeGraphDataLike` + `graphDataStore` 注册表 enrich 写入 store。
 */
export type GraphDataLike = GraphData | GraphDataInput | Graph | GraphInstanceDTO;
