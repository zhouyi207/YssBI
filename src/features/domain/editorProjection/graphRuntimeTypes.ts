/**
 * Normalized editor-projection runtime structures.
 *
 * `graphProjectionStore` buckets are editor projections. `replaceProjection` is the
 * only bucket creation path and always installs projection basis, revision,
 * request generation, and diagnostics together with normalized entities.
 */

import type { ConnectionId, GraphPath, NodeId, PinId } from "@/shared/types/domain/ids";
export type { NodeId, PinId, GraphPath, ConnectionId };
import type { PinDirection } from "@/shared/types/domain/pin";
import type { DataType } from "@/shared/types/domain/dataType";
import type {
  DiagnosticDto,
  EditorInputBindingDto,
  NodeCapabilitiesDto,
  NodeDisplayDto,
  ParameterEditorDto,
  PortInstanceAdditionDto,
  PortAddressDto,
  PortConnectionCapabilityDto,
  PortDisplayDto,
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
  pinIds: string[];
  position: { x: number; y: number };
  display: NodeDisplayDto;
  parameterEditors: ParameterEditorDto[];
  portInstanceAdditions: PortInstanceAdditionDto[];
  capabilities: NodeCapabilitiesDto;
  /** Node-local diagnostic index; the graph bucket owns the canonical problem set. */
  diagnostics: DiagnosticDto[];
}

// ==================== PinData ====================
/** Pin 实体数据。连接关系不在这里保存；运行时连接状态从 pinConnections 派生。 */
export interface PinData {
  id: string;
  nodeId: string;
  name: string;
  direction: PinDirection;
  /** Derived display/interaction alias for `resolvedType.dataType`. */
  dataType?: DataType;
  /** Stable structured address; projected pins use its local key as `id`. */
  address: PortAddressDto;
  display: PortDisplayDto;
  orphan: boolean;
  canRemove: boolean;
  connections: PortConnectionCapabilityDto;
  input: EditorInputBindingDto | null;
  resolvedType: TypeSummaryDto | null;
  resolvedSchema: SchemaSummaryDto | null;
  status: ResolvedPortStatusDto;
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
