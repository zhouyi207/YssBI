import type { NodePositionDto } from "@/shared/types/domain/editorProjection";
import type { TypeExprDto, TypedLiteralDto } from "@/shared/types/domain/editorMutation";

export type ClipboardNodeCreationDto =
  | { kind: "static"; nodeTypeId: string }
  | {
      kind: "resourceBound";
      nodeTypeId: string;
      resourcePath: string;
      createArgs: ClipboardResourceBoundCreateArgsDto;
    };

export type ClipboardResourceBoundCreateArgsDto =
  | { kind: "function" }
  | { kind: "variable" }
  | { kind: "database" };

export type ClipboardPortRefDto =
  | { kind: "declared"; key: string }
  | { kind: "instance"; template: string; localInstanceId: string };

export interface ClipboardPortAddressDto {
  nodeId: string;
  port: ClipboardPortRefDto;
}

export interface ClipboardNodeDto {
  localId: string;
  creation: ClipboardNodeCreationDto;
  parameters: Record<string, unknown>;
  userLabel: string | null;
  relativePosition: NodePositionDto;
}

export type ClipboardDynamicMemberOriginDto =
  | { kind: "functionParameter"; function: string; parameter: string }
  | { kind: "schemaField"; source: string; field: string };

export interface ClipboardLastKnownPortMetadataDto {
  label: string;
  valueType?: TypeExprDto;
}

export type ClipboardDynamicPortBindingDto =
  | { kind: "userCreated"; order: string }
  | {
      kind: "resolved" | "orphan";
      origin: ClipboardDynamicMemberOriginDto;
      order: string;
      lastKnown: ClipboardLastKnownPortMetadataDto;
    };

export interface ClipboardPortBindingDto {
  address: ClipboardPortAddressDto;
  binding: ClipboardDynamicPortBindingDto;
}

export interface ClipboardInputStateDto {
  address: ClipboardPortAddressDto;
  state: { literalOverride: TypedLiteralDto | null };
}

export interface ClipboardConnectionDto {
  output: ClipboardPortAddressDto;
  input: ClipboardPortAddressDto;
  order: string | null;
}

export interface ClipboardSubgraphDto {
  schemaVersion: 1;
  nodes: ClipboardNodeDto[];
  portBindings: ClipboardPortBindingDto[];
  inputStates: ClipboardInputStateDto[];
  connections: ClipboardConnectionDto[];
}
