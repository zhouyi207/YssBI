/**
 * Normalized editor-projection runtime structures.
 *
 * `graphDataStore` buckets are editor projections. `replaceProjection` is the
 * only bucket creation path and always installs projection basis, revision,
 * request generation, and diagnostics together with normalized entities.
 */

import type { ConnectionId, GraphPath, NodeId, PinId } from "@/shared/types/domain/ids";
export type { NodeId, PinId, GraphPath, ConnectionId };
import type { PinDirection, PinUI, RuntimePinKind } from "@/shared/types/domain/pin";
import type { DataType } from "@/shared/types/domain/dataType";
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
} from "@/shared/types/domain/editorProjection";

// ==================== NodeData ====================
/** 节点数据（Store 规范化格式，camelCase 与 DESIGN_RULE 一致） */
export interface NodeData {
  id: string;
  graphPath: string;
  nodeType: string;
  category: string[];
  title: string;
  inputs: string[]; // Pin IDs
  outputs: string[]; // Pin IDs
  position: { x: number; y: number };
  /** Backend-authored editor projection fields. */
  display?: NodeDisplayDto;
  parameterEditors?: ParameterEditorDto[];
  capabilities?: NodeCapabilitiesDto;
  diagnostics?: DiagnosticDto[];
  /** 以下为 UI 扩展字段 */
  isInternal?: boolean;
  /** UI projection discriminator; not a creation identity. */
  paramsKind?: "none" | "variable" | "subGraph" | "dataFrame";
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
/** Editor projection connection keyed by its backend-authored stable identity. */
export interface ConnectionData {
  id: string;
  /** Local key derived from the output port's structured address. */
  from: string;
  /** Local key derived from the input port's structured address. */
  to: string;
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
  type: "event" | "function";
  functionInputs?: import("@/shared/types/domain/graph").FunctionSignaturePin[];
  functionOutputs?: import("@/shared/types/domain/graph").FunctionSignaturePin[];
  nodes: NodeData[];
  pins: PinData[];
  connections: ConnectionData[];
}

/** Export snapshot shape; domain serialization needs endpoint pairs, not runtime connection identity. */
export type GraphSnapshotData = Omit<GraphData, "connections"> & {
  connections: Array<Pick<ConnectionData, "from" | "to">>;
};
