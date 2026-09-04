import type { EditorGraphProjectionDto, PortAddressDto } from "@/shared/types/dto/editorProjection";
import type { GraphEditorSessionDto } from "@/shared/types/dto/editorMutation";
import type { DataType } from "@/shared/types/domain/dataType";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { portAddressKey } from "@/features/domain/editorProjection";

export interface EditorProjectionFixtureOptions {
  graphPath: string;
  nodeId?: string;
  nodeTypeId?: string;
  title?: string;
  connectionId?: string;
}

export function makeProjectedPinData(
  overrides: Partial<PinData> &
    Pick<PinData, "id" | "nodeId" | "direction"> & { dataType?: DataType },
): PinData {
  const { dataType: overriddenDataType, ...projectedOverrides } = overrides;
  const dataType: DataType | undefined = overriddenDataType ?? { kind: "Float64" };
  const label = overrides.name ?? overrides.id;
  const base: PinData = {
    id: overrides.id,
    nodeId: overrides.nodeId,
    name: label,
    direction: overrides.direction,
    address: {
      kind: "declared",
      nodeId: overrides.nodeId,
      portKey: overrides.id,
    },
    display: { label, instanceLabel: null },
    orphan: false,
    canRemove: false,
    connections: {
      current: 0,
      maximum: overrides.direction === "input" ? 1 : null,
      ordered: false,
      canAppend: true,
      canReplace: false,
      canMove: true,
    },
    input:
      overrides.direction === "input"
        ? { literalOverride: null, protocolDefault: null, effective: "unbound" }
        : null,
    acceptedType: overrides.acceptedType ?? {
      display: dataType?.kind ?? "unknown",
      domain: dataType ? [dataType] : null,
    },
    typeState:
      overrides.typeState ??
      (dataType
        ? { status: "exact", display: dataType.kind, dataType }
        : { status: "unknown", reasonCode: "unsupported_declaration" }),
    resolvedSchema: null,
    status: "resolved",
  };
  return { ...base, ...projectedOverrides };
}

export function makeGraphEditorSession(
  projection: EditorGraphProjectionDto,
): GraphEditorSessionDto {
  return {
    document: {
      nodes: {},
      port_bindings: [],
      connections: {},
      input_states: [],
    },
    projection,
  };
}

export function makeEditorProjectionFixture(options: EditorProjectionFixtureOptions): {
  projection: EditorGraphProjectionDto;
  inputAddress: PortAddressDto;
  inputKey: string;
  outputAddress: PortAddressDto;
  outputKey: string;
} {
  const {
    graphPath,
    nodeId = "local-node",
    nodeTypeId = "tests.projected-node",
    title = "Projected node",
    connectionId = "local-connection",
  } = options;
  const outputAddress: PortAddressDto = {
    kind: "declared",
    nodeId,
    portKey: "local-out",
  };
  const inputAddress: PortAddressDto = {
    kind: "declared",
    nodeId,
    portKey: "local-in",
  };
  const outputKey = portAddressKey(outputAddress);
  const inputKey = portAddressKey(inputAddress);

  return {
    outputAddress,
    outputKey,
    inputAddress,
    inputKey,
    projection: {
      basis: {
        graphPath,
        registryFingerprint: "0000000000000000000000000000000000000000000000000000000000000000",
        resourceVersions: {},
      },
      graphPath,
      nodes: [
        {
          graphPath,
          nodeId,
          nodeTypeId,
          position: { x: 0, y: 0 },
          display: {
            title,
            userLabel: null,
            iconId: null,
            styleId: null,
          },
          ports: [
            {
              address: outputAddress,
              display: { label: "Output", instanceLabel: null },
              direction: "output",
              orphan: false,
              canRemove: false,
              connections: {
                current: 1,
                maximum: null,
                ordered: false,
                canAppend: true,
                canReplace: false,
                canMove: true,
              },
              input: null,
              acceptedType: { display: "Float64", domain: [{ kind: "Float64" }] },
              typeState: {
                status: "exact",
                display: "Float64",
                dataType: { kind: "Float64" },
              },
              resolvedSchema: null,
              status: "resolved",
            },
            {
              address: inputAddress,
              display: { label: "Input", instanceLabel: null },
              direction: "input",
              orphan: false,
              canRemove: false,
              connections: {
                current: 1,
                maximum: 1,
                ordered: false,
                canAppend: false,
                canReplace: true,
                canMove: true,
              },
              input: {
                literalOverride: null,
                protocolDefault: null,
                effective: "connections",
              },
              acceptedType: { display: "Float64", domain: [{ kind: "Float64" }] },
              typeState: {
                status: "exact",
                display: "Float64",
                dataType: { kind: "Float64" },
              },
              resolvedSchema: null,
              status: "resolved",
            },
          ],
          portInstanceAdditions: [],
          parameterEditors: [],
          capabilities: {
            managed: false,
            canCopy: true,
            canDelete: true,
            canEditLabel: true,
            canEditParameters: false,
            supportsInlineLiterals: false,
          },
          diagnostics: [],
        },
      ],
      connections: [
        {
          connectionId,
          output: outputAddress,
          input: inputAddress,
          order: null,
        },
      ],
      diagnostics: [],
      outcome: { type: "success" },
      hasBlockingDiagnostics: false,
    },
  };
}
