import type { EditorGraphProjectionDto, PortAddressDto } from "@/shared/types/dto/editorProjection";
import type {
  GraphEditorSessionDto,
  GraphEditingStateDto,
  GraphConstantDto,
} from "@/shared/types/dto/editorMutation";
import type { ValueType } from "@/shared/types/domain/valueType";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { portAddressKey } from "@/features/domain/editorProjection";
import { useResourceStore } from "@/features/core/resource/resourceStore";

export function makeGraphConstantFixture(id: string) {
  return {
    id,
    name: `Constant ${id}`,
    dataType: { kind: "Object" },
    dataValue: {
      Object: {
        left: { List: Array.from({ length: 16 }, (_, index) => ({ Integer: String(index) })) },
        right: {
          List: Array.from({ length: 16 }, (_, index) => ({ Integer: String(index + 16) })),
        },
        metadata: { Object: { label: { String: "samples" }, enabled: { Bool: true } } },
      },
    },
    description: "Nested constant fixture",
    tags: ["fixture", "nested"],
  } satisfies GraphConstantDto;
}

/** Ordinary JSON used by parameter values and input bindings, not serialized DataValue variants. */
export function makeGraphProjectionJsonFixture(key: string) {
  return {
    left: Array.from({ length: 16 }, (_, index) => ({ index, value: `${key}-${index}` })),
    right: Array.from({ length: 16 }, (_, index) => ({ index, value: `kept-${index}` })),
    nested: { key },
    metadata: {
      label: "samples",
      enabled: true,
      ["__proto__"]: { items: ["first", "second"] },
      constructor: { keep: { key: "kept" }, change: { key: "old" } },
    },
  };
}

export function changeGraphProjectionJsonFixture(
  value: ReturnType<typeof makeGraphProjectionJsonFixture>,
) {
  value.left[0].value = "changed";
  value.nested.key = "changed";
  value.metadata.__proto__.items.reverse();
  value.metadata.constructor.change.key = "changed";
  delete (value.metadata as Partial<typeof value.metadata>).enabled;
  Object.assign(value.metadata, { added: { key: "added" } });
}

export function installGraphProjectionFixture(
  graphPath: string,
  projection: EditorGraphProjectionDto,
): void {
  const store = useResourceStore.getState();
  const previous = store.sessions[graphPath];
  const session = makeGraphEditorSession(projection);
  if (previous) {
    session.document.constants = previous.constants;
    session.editing = {
      version: previous.version,
      dirty: previous.saveDirty,
      canUndo: previous.canUndo,
      canRedo: previous.canRedo,
    };
  }
  store.installGraphSession(graphPath, session);
}

export interface EditorProjectionFixtureOptions {
  graphPath: string;
  nodeId?: string;
  nodeTypeId?: string;
  title?: string;
  connectionId?: string;
}

export function makeProjectedPinData(
  overrides: Partial<PinData> &
    Pick<PinData, "id" | "nodeId" | "direction"> & { dataType?: ValueType },
): PinData {
  const { dataType: overriddenDataType, ...projectedOverrides } = overrides;
  const dataType: ValueType = overriddenDataType ?? {
    kind: "Scalar",
    inner: "Numeric",
  };
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
      canAppend: true,
      canReplace: false,
      canMove: false,
    },
    input:
      overrides.direction === "input"
        ? {
            literalOverride: null,
            protocolDefault: null,
            effective: "unbound",
          }
        : null,
    typeState: overrides.typeState ?? { status: "exact", display: dataType.kind, dataType },
    resolvedSchema: null,
    status: "resolved",
  };
  return { ...base, ...projectedOverrides };
}

export function makeGraphEditorSession(
  projection: EditorGraphProjectionDto,
): GraphEditorSessionDto {
  return {
    editing: makeGraphEditingState(),
    document: {
      nodes: {},
      port_bindings: [],
      connections: {},
      input_states: [],
    },
    projection,
    resultState: {
      revision: "0",
      executionSessionId: "00000000-0000-0000-0000-000000000091",
      semanticInputHash: projection.basis.semanticInputHash,
      outputs: projection.nodes.flatMap((node) =>
        node.ports
          .filter((port) => port.direction === "output")
          .map((port) => ({
            output: { graphPath: projection.graphPath, port: port.address },
            state: "missing" as const,
            resultId: null,
          })),
      ),
      connections: [],
    },
  };
}

export function makeGraphEditingState(
  overrides: Partial<GraphEditingStateDto> = {},
): GraphEditingStateDto {
  return {
    version: { sessionId: "00000000-0000-0000-0000-000000000090", revision: "0" },
    dirty: false,
    canUndo: false,
    canRedo: false,
    ...overrides,
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

        semanticInputHash: "0".repeat(64),

        resourceObservations: {},
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
                canAppend: true,
                canReplace: false,
                canMove: true,
              },
              input: null,
              typeState: {
                status: "exact",
                display: "Float64",
                dataType: { kind: "Scalar", inner: "Numeric" },
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
                canAppend: false,
                canReplace: true,
                canMove: true,
              },
              input: {
                literalOverride: null,
                protocolDefault: null,
                effective: "connections",
              },
              typeState: {
                status: "exact",
                display: "Float64",
                dataType: { kind: "Scalar", inner: "Numeric" },
              },
              resolvedSchema: null,
              status: "resolved",
            },
          ],
          portInstanceAdditions: [],
          parameterGroups: [],
          capabilities: {
            managed: false,
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
