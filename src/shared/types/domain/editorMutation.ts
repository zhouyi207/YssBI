import type { ChartDocumentState } from "@/shared/types/domain/chart";
import type { DatabaseDocumentDto } from "@/shared/types/domain/database";
import type { NodeCreationDescriptorDto } from "@/shared/types/domain/nodeCreationDescriptor";
import type {
  EditorGraphProjectionDto,
  GraphProjectionReplacementDto,
  NodePositionDto,
  PortAddressDto,
} from "@/shared/types/domain/editorProjection";

export type ResourceKeyDto =
  | { kind: "graph"; key: string }
  | { kind: "function"; key: string }
  | { kind: "variable"; key: string }
  | { kind: "database"; key: string }
  | { kind: "chart"; key: string };

export interface MutationRequestDto<TPayload> {
  resource: ResourceKeyDto;
  baseRevision: number;
  operationId: string;
  payload: TPayload;
}

export type EditorGraphMutationDto =
  | {
      type: "createNode";
      payload: {
        descriptor: NodeCreationDescriptorDto;
        position: NodePositionDto;
        userLabel: string | null;
        connectFrom: PortAddressDto | null;
      };
    }
  | { type: "deleteNodes"; payload: { nodeIds: string[] } }
  | {
      type: "setParameters";
      payload: { nodeId: string; parameters: Record<string, unknown> };
    }
  | {
      type: "moveNodes";
      payload: { positions: Array<{ nodeId: string; position: NodePositionDto }> };
    }
  | {
      type: "connect";
      payload: { output: PortAddressDto; input: PortAddressDto; order: string | null };
    }
  | { type: "disconnectConnections"; payload: { connectionIds: string[] } }
  | {
      type: "insertReroute";
      payload: {
        connectionId: string;
        position: NodePositionDto;
      };
    }
  | { type: "disconnectPort"; payload: { address: PortAddressDto } }
  | { type: "disconnectNode"; payload: { nodeId: string } }
  | {
      type: "moveConnections";
      payload: { source: PortAddressDto; target: PortAddressDto };
    }
  | { type: "setLiteral"; payload: { address: PortAddressDto; literal: unknown | null } }
  | {
      type: "addPortInstance";
      payload: { nodeId: string; templateKey: string; order: string | null };
    }
  | { type: "removePortInstance"; payload: { address: PortAddressDto } }
  | {
      type: "duplicateSubgraph";
      payload: { nodeIds: string[]; offset: NodePositionDto };
    }
  | {
      type: "insertSubgraph";
      payload: { snapshotJson: string; anchor: NodePositionDto };
    };

export type HistoryMutationDto = Record<string, never>;

export interface HistoryStatusDto {
  canUndo: boolean;
  canRedo: boolean;
}

export type DocumentPortAddressDto = {
  node_id: string;
  port:
    | { kind: "declared"; key: string }
    | { kind: "instance"; template: string; instance_id: string };
};

export interface DocumentNodeDto {
  id: string;
  node_type: string;
  position: NodePositionDto;
  parameters: Record<string, unknown>;
  user_label: string | null;
}

export interface DocumentConnectionDto {
  id: string;
  output: DocumentPortAddressDto;
  input: DocumentPortAddressDto;
  order: string | null;
}

export type DynamicMemberLocatorDto =
  | {
      kind: "function_parameter";
      function: string;
      parameter: string;
    }
  | {
      kind: "schema_field";
      source: string;
      field: string;
    };

export type TypeExprDto =
  | { Concrete: string }
  | { Class: string }
  | { Generic: string }
  | { Applied: { constructor: string; arguments: TypeExprDto[] } }
  | { Union: TypeExprDto[] }
  | "Unknown";

export type ProtocolValueDto =
  | "Null"
  | { Bool: boolean }
  | { Integer: number }
  | { Unsigned: number }
  | { Decimal: string }
  | { String: string }
  | { Bytes: number[] }
  | { List: ProtocolValueDto[] }
  | { Object: Record<string, ProtocolValueDto> };

export interface TypedLiteralDto {
  value_type: TypeExprDto;
  value: ProtocolValueDto;
}

export type DynamicPortBindingDto =
  | { kind: "user_created"; order: string }
  | {
      kind: "resolved";
      origin: DynamicMemberLocatorDto;
      order: string;
      last_known?: { label: string; value_type?: TypeExprDto };
    }
  | {
      kind: "orphan";
      origin: DynamicMemberLocatorDto;
      order: string;
      last_known: { label: string; value_type?: TypeExprDto };
    };

export interface InputStateDto {
  literal_override: TypedLiteralDto | null;
}

/** Raw unsaved Graph document owned by one frontend editor session. */
export interface GraphDocumentDto {
  nodes: Record<string, DocumentNodeDto>;
  port_bindings: Array<[DocumentPortAddressDto, DynamicPortBindingDto]>;
  connections: Record<string, DocumentConnectionDto>;
  input_states: Array<[DocumentPortAddressDto, InputStateDto]>;
}

export type GraphDocumentOperationDto =
  | { operation: "insert_node"; node: DocumentNodeDto }
  | { operation: "remove_node"; node: DocumentNodeDto }
  | { operation: "update_node"; before: DocumentNodeDto; after: DocumentNodeDto }
  | {
      operation: "insert_port_binding" | "remove_port_binding";
      address: DocumentPortAddressDto;
      binding: DynamicPortBindingDto;
    }
  | { operation: "insert_connection" | "remove_connection"; connection: DocumentConnectionDto }
  | {
      operation: "set_input_state";
      address: DocumentPortAddressDto;
      before: InputStateDto | null;
      after: InputStateDto | null;
    };

export interface GraphDocumentPatchDto {
  operations: GraphDocumentOperationDto[];
}

export interface FunctionParameterDto {
  id: string;
  name: string;
  type_name: string;
}

export interface FunctionSignatureDto {
  parameters: FunctionParameterDto[];
  return_type: string | null;
}

export interface FunctionDocumentPatchDto {
  before: FunctionSignatureDto;
  after: FunctionSignatureDto;
}

export interface VariableDocumentPatchDto {
  before: unknown;
  after: unknown;
}

export interface DatabaseDocumentPatchDto {
  before: DatabaseDocumentDto | null;
  after: DatabaseDocumentDto | null;
}

export interface ChartDocumentPatchDto {
  before: ChartDocumentState;
  after: ChartDocumentState;
}

export interface ResourcePathMovePatchDto {
  from: string;
  to: string;
}

export type ResourceLifecycleKindDto = "event" | "function" | "chart";

export interface ResourceLifecycleStateDto {
  revision: number;
  path: string;
  kind: ResourceLifecycleKindDto;
  name: string;
}

export interface ResourceLifecyclePatchDto {
  before: ResourceLifecycleStateDto | null;
  after: ResourceLifecycleStateDto | null;
}

export type ResourceDocumentPatchDto =
  | { kind: "graph"; patch: GraphDocumentPatchDto }
  | { kind: "function"; patch: FunctionDocumentPatchDto }
  | { kind: "chart"; patch: ChartDocumentPatchDto }
  | { kind: "resource_lifecycle"; patch: ResourceLifecyclePatchDto }
  | { kind: "resource_move"; patch: ResourcePathMovePatchDto }
  | { kind: "variable"; patch: VariableDocumentPatchDto }
  | { kind: "variable_scope_move"; patch: ResourcePathMovePatchDto }
  | { kind: "database"; patch: DatabaseDocumentPatchDto };

export interface ResourceDeltaDto<TPayload = ResourceDocumentPatchDto> {
  resource: ResourceKeyDto;
  fromRevision: number;
  toRevision: number;
  causedBy: string | null;
  payload: TPayload;
}

export interface GraphEditorSessionDto {
  document: GraphDocumentDto;
  projection: EditorGraphProjectionDto;
}

export interface CompileGraphDraftDto {
  sourceHash: string;
  cacheHit: boolean;
  document: GraphDocumentDto;
  projection: EditorGraphProjectionDto;
}

export interface GraphDraftTransformDto {
  changed: boolean;
  document: GraphDocumentDto;
  projection: EditorGraphProjectionDto;
}

export interface GraphDraftSaveDto {
  projectInstanceId: string;
  operationId: string;
  resourceRevision: number;
  document: GraphDocumentDto;
  projectionReplacement: GraphProjectionReplacementDto;
  history: HistoryStatusDto;
}

export type ProjectionStatusDto =
  | { status: "complete"; expectedGraphPaths: string[] }
  | { status: "incomplete"; invalidatedGraphPaths: string[] };

export interface ResourceMoveDto {
  from: string;
  to: string;
  kind: ResourceLifecycleKindDto;
  name: string;
}

export interface ResourceMutationResultDto {
  operationId: string;
  projectInstanceId: string;
  publicationRevision: number;
  moves: ResourceMoveDto[];
  deltas: ResourceDeltaDto[];
  projectionReplacements: GraphProjectionReplacementDto[];
  projectionStatus: ProjectionStatusDto;
  history: HistoryStatusDto;
}

export type { EditorGraphProjectionDto, GraphProjectionReplacementDto };
