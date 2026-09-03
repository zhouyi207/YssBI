import type { EditorGraphProjectionDto, PortAddressDto } from "@/shared/types/dto/editorProjection";
import type { GraphEditorSessionDto } from "@/shared/types/dto/editorMutation";
import type { DataType } from "@/shared/types/domain/dataType";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { portAddressKey } from "@/features/domain/editorProjection";

export interface EditorProjectionFixtureOptions {
  graphPath: string;
  sourceRevision?: number;
  nodeId?: string;
  nodeTypeId?: string;
  title?: string;
  connectionId?: string;
}

export function makeProjectedPinData(
  overrides: Partial<PinData> & Pick<PinData, "id" | "nodeId" | "direction">,
): PinData {
  const kind = overrides.kind ?? "data";
  const dataType: DataType | undefined =
    kind === "data" ? (overrides.dataType ?? { kind: "Float64" }) : undefined;
  const label = overrides.name ?? overrides.id;
  const base: PinData = {
    id: overrides.id,
    nodeId: overrides.nodeId,
    name: label,
    direction: overrides.direction,
    dataType,
    address: {
      kind: "declared",
      nodeId: overrides.nodeId,
      portKey: overrides.id,
    },
    display: { label, instanceLabel: null },
    kind,
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
    resolvedType: dataType ? { display: dataType.kind, resolved: true, dataType } : null,
    resolvedSchema: null,
    status: "resolved",
  };
  return { ...base, ...overrides };
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
    sourceRevision = 1,
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
      sourceRevision,
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
              kind: "data",
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
              resolvedType: { display: "Float64", resolved: true, dataType: { kind: "Float64" } },
              resolvedSchema: null,
              status: "resolved",
            },
            {
              address: inputAddress,
              display: { label: "Input", instanceLabel: null },
              direction: "input",
              kind: "data",
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
              resolvedType: { display: "Float64", resolved: true, dataType: { kind: "Float64" } },
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
