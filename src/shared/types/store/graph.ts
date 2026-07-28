/**
 * Store-layer graph data structures.
 *
 * `graphDataStore` buckets are editor projections. `replaceProjection` is the
 * only bucket creation path and always installs projection basis, revision,
 * request generation, and diagnostics together with normalized entities.

 * `GraphData`, `GraphDataInput`, and `GraphDataLike` remain legacy DTO/domain
 * conversion types used outside the projection store boundary.
 */

import type { NodeId, PinId, GraphPath, ConnectionId } from '../domain/ids';
export type { NodeId, PinId, GraphPath, ConnectionId };
import type { Graph } from '../domain/graph';
import type { PinDirection, PinUI, RuntimePinKind } from '../domain/pin';
import type { DataType } from '../domain/dataType';
import type { GraphInstanceDTO } from '../dto/graph';
import type {
  DiagnosticDto,
  EditorInputBindingDto,
  NodeCapabilitiesDto,
  NodeDisplayDto,
  ParameterEditorDto,
  PortAddressDto,
  PortConnectionCapabilityDto,
  PortDisplayDto,
  PortInstanceKindDto,
  PortKindDto,
  ResolvedPortStatusDto,
  SchemaSummaryDto,
  TypeSummaryDto,
} from '../dto/editorProjection';

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
  /** Backend-authored editor projection fields. */
  display?: NodeDisplayDto;
  parameterEditors?: ParameterEditorDto[];
  capabilities?: NodeCapabilitiesDto;
  diagnostics?: DiagnosticDto[];
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
  /** Stable structured address; projected pins use its local key as `id`. */
  address?: PortAddressDto;
  templateKey?: string;
  display?: PortDisplayDto;
  kind?: PortKindDto;
  instanceKind?: PortInstanceKindDto;
  orphan?: boolean;
  canRemove?: boolean;
  connections?: PortConnectionCapabilityDto;
  input?: EditorInputBindingDto | null;
  resolvedType?: TypeSummaryDto | null;
  resolvedSchema?: SchemaSummaryDto | null;
  status?: ResolvedPortStatusDto;
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
  id: string;   // Legacy: "fromPinId->toPinId"; projection: stable connection id
  from: string; // Local PortAddressKey / legacy PinId
  to: string;   // Local PortAddressKey / legacy PinId
  output?: PortAddressDto;
  input?: PortAddressDto;
  order?: string | null;
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

/** Legacy graph-body input accepted by DTO/domain conversion utilities outside the projection store. */
export type GraphDataLike = GraphData | GraphDataInput | Graph | GraphInstanceDTO;
