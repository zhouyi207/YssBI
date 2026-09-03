import { describe, expect, it } from "vitest";
import type { EditorGraphMutationDto } from "./editorMutation";
import {
  parseEditorGraphMutationDto,
  parseGraphProjectionReplacementDto,
} from "./editorMutationWireParser";

const graphPath = "events/Main.yssbi-event";
const functionPath = "functions/Forecast.yssbi-function";
const nodeId = "00000000-0000-0000-0000-000000000101";
const connectionId = "00000000-0000-0000-0000-000000000201";
const projectedDeclaredAddress = { kind: "declared" as const, nodeId, portKey: "value" };
const phase1Mutations = [
  { type: "deleteNodes", payload: { nodeIds: [nodeId] } },
  { type: "disconnectConnections", payload: { connectionIds: [connectionId] } },
  { type: "disconnectPort", payload: { address: projectedDeclaredAddress } },
  { type: "disconnectNode", payload: { nodeId } },
  {
    type: "moveConnections",
    payload: { source: projectedDeclaredAddress, target: projectedDeclaredAddress },
  },
] satisfies EditorGraphMutationDto[];
function projection(path: string, revision: number) {
  return {
    basis: {
      graphPath: path,
      registryFingerprint: "0".repeat(64),
      resourceVersions: {},
    },
    graphPath: path,
    sourceRevision: revision,
    nodes: [],
    connections: [],
    diagnostics: [],
    outcome: { type: "success" },
    hasBlockingDiagnostics: false,
  };
}

function projectionWithParameterEditor(): Record<string, unknown> {
  const value = projectionWithResolvedType(null);
  const projectedNode = (value.nodes as Array<Record<string, unknown>>)[0];
  projectedNode.parameterEditors = [
    {
      key: "value",
      display: { title: "Value", description: null },
      editor: "number",
      presentation: "inlineAndDetail",
      valueType: { kind: "Int64" },
      multiline: false,
      value: 1,
      configuration: null,
      inheritedValue: null,
      valueSource: null,
      options: null,
    },
  ];
  return value;
}

function projectionWithResolvedType(resolvedType: unknown): Record<string, unknown> {
  const value: Record<string, unknown> = projection(graphPath, 5);
  value.nodes = [
    {
      graphPath,
      nodeId,
      nodeTypeId: "core.constant",
      position: { x: 1, y: 2 },
      display: {
        title: "Constant",
        userLabel: null,
        iconId: null,
        styleId: null,
      },
      ports: [
        {
          address: { kind: "declared", nodeId, portKey: "value" },
          display: { label: "Value", instanceLabel: null },
          direction: "output",
          kind: "data",
          orphan: false,
          canRemove: false,
          connections: {
            current: 0,
            maximum: null,
            ordered: false,
            canAppend: true,
            canReplace: false,
            canMove: false,
          },
          input: null,
          resolvedType,
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
  ];
  return value;
}

function functionEditorProjection(revision = 5) {
  return {
    functionRevision: revision,
    inputs: [{ id: "sales", name: "Observed sales", dataType: { kind: "Float64" } }],
    outputs: [
      {
        id: "return",
        name: "Array<String>",
        dataType: { kind: "Array", inner: { kind: "String" } },
      },
    ],
  };
}

describe("editor mutation wire parser", () => {
  it("parses the exact InsertReroute DTO wire shape", () => {
    const mutation = {
      type: "insertReroute",
      payload: {
        connectionId: "edge-1",
        position: { x: 120, y: 80 },
      },
    };

    expect(parseEditorGraphMutationDto(mutation)).toEqual(mutation);
  });

  it.each([
    { type: "insertReroute", payload: { connectionId: "", position: { x: 120, y: 80 } } },
    { type: "insertReroute", payload: { connectionId: "   ", position: { x: 120, y: 80 } } },
    { type: "unknownReroute", payload: { connectionId: "edge-1", position: { x: 120, y: 80 } } },
    {
      type: "insertReroute",
      payload: { connectionId: "edge-1", position: { x: Infinity, y: 80 } },
    },
    {
      type: "insertReroute",
      payload: { connectionId: "edge-1", position: { x: -Infinity, y: 80 } },
    },
    { type: "insertReroute", payload: { connectionId: "edge-1", position: { x: 120, y: NaN } } },
    { type: "insertReroute", payload: { connectionId: "edge-1", position: { x: "120", y: 80 } } },
    { type: "insertReroute", payload: { connectionId: 1, position: { x: 120, y: 80 } } },
    { type: "insertReroute", payload: { connectionId: "edge-1", position: { x: 120 } } },
    {
      type: "insertReroute",
      payload: { connectionId: "edge-1", position: { x: 120, y: 80, z: 0 } },
    },
    {
      type: "insertReroute",
      payload: { connectionId: "edge-1", position: { x: 120, y: 80 }, extra: true },
    },
    {
      type: "insertReroute",
      payload: { connectionId: "edge-1", position: { x: 120, y: 80 } },
      extra: true,
    },
  ])("rejects malformed InsertReroute DTO wire shape %#", (mutation) => {
    expect(() => parseEditorGraphMutationDto(mutation)).toThrow("InsertReroute");
  });

  it("exposes all Phase 1 collection and connection intent DTO variants", () => {
    expect(phase1Mutations.map((mutation) => mutation.type)).toEqual([
      "deleteNodes",
      "disconnectConnections",
      "disconnectPort",
      "disconnectNode",
      "moveConnections",
    ]);
  });

  it("requires all six exact connection capability fields", () => {
    const valid = projectionWithResolvedType({
      display: "Float64",
      resolved: true,
      dataType: { kind: "Float64" },
    });
    const node = (valid.nodes as Array<Record<string, unknown>>)[0];
    const port = (node.ports as Array<Record<string, unknown>>)[0];
    const capability = port.connections as Record<string, unknown>;

    expect(parseGraphProjectionReplacementDto({ graphPath, projection: valid })).toEqual({
      graphPath,
      projection: valid,
    });

    for (const key of ["current", "maximum", "ordered", "canAppend", "canReplace", "canMove"]) {
      const malformed = structuredClone(valid);
      const malformedNode = (malformed.nodes as Array<Record<string, unknown>>)[0];
      const malformedPort = (malformedNode.ports as Array<Record<string, unknown>>)[0];
      delete (malformedPort.connections as Record<string, unknown>)[key];
      expect(() =>
        parseGraphProjectionReplacementDto({ graphPath, projection: malformed }),
      ).toThrow("projection replacement");
    }

    for (const key of ["canAppend", "canReplace", "canMove"]) {
      const malformed = structuredClone(valid);
      const malformedNode = (malformed.nodes as Array<Record<string, unknown>>)[0];
      const malformedPort = (malformedNode.ports as Array<Record<string, unknown>>)[0];
      (malformedPort.connections as Record<string, unknown>)[key] = "yes";
      expect(() =>
        parseGraphProjectionReplacementDto({ graphPath, projection: malformed }),
      ).toThrow("projection replacement");
    }

    capability.extra = false;
    expect(() => parseGraphProjectionReplacementDto({ graphPath, projection: valid })).toThrow(
      "projection replacement",
    );
  });

  it("requires strict parameter editor presentation metadata", () => {
    const replacement = {
      graphPath,
      projection: projectionWithParameterEditor(),
    };
    expect(parseGraphProjectionReplacementDto(replacement)).toEqual(replacement);

    const missing = structuredClone(replacement) as any;
    delete missing.projection.nodes[0].parameterEditors[0].presentation;
    expect(() => parseGraphProjectionReplacementDto(missing)).toThrow("projection replacement");

    const invalid = structuredClone(replacement) as any;
    invalid.projection.nodes[0].parameterEditors[0].presentation = "inlineOnly";
    expect(() => parseGraphProjectionReplacementDto(invalid)).toThrow("projection replacement");
  });

  it.each([
    [
      "missing inheritedValue",
      (editor: Record<string, unknown>) => {
        delete editor.inheritedValue;
      },
    ],
    [
      "missing valueSource",
      (editor: Record<string, unknown>) => {
        delete editor.valueSource;
      },
    ],
    [
      "missing options",
      (editor: Record<string, unknown>) => {
        delete editor.options;
      },
    ],
    [
      "invalid valueSource casing",
      (editor: Record<string, unknown>) => {
        editor.valueSource = "Project";
      },
    ],
    [
      "non-string options",
      (editor: Record<string, unknown>) => {
        editor.options = [1];
      },
    ],
    [
      "missing valueType",
      (editor: Record<string, unknown>) => {
        delete editor.valueType;
      },
    ],
    [
      "string valueType",
      (editor: Record<string, unknown>) => {
        editor.valueType = "Int64";
      },
    ],
    [
      "malformed valueType",
      (editor: Record<string, unknown>) => {
        editor.valueType = { kind: "Array" };
      },
    ],
    [
      "valueType with an extra key",
      (editor: Record<string, unknown>) => {
        editor.valueType = { kind: "Int64", extra: true };
      },
    ],
    [
      "parameter editor with an extra key",
      (editor: Record<string, unknown>) => {
        editor.extra = true;
      },
    ],
  ])("rejects parameter editor %s", (_, mutate) => {
    const replacement = {
      graphPath,
      projection: projectionWithParameterEditor(),
    };
    const projectedNode = (replacement.projection.nodes as Array<Record<string, unknown>>)[0];
    const editor = (projectedNode.parameterEditors as Array<Record<string, unknown>>)[0];
    mutate(editor);
    expect(() => parseGraphProjectionReplacementDto(replacement)).toThrow("projection replacement");
  });

  it("requires an exact structured dataType on every non-null resolved type", () => {
    const malformedResolvedTypes = [
      { display: "Float64", resolved: true },
      { display: "Float64", resolved: true, dataType: "Float64" },
      { display: "Float64", resolved: true, dataType: { kind: "Float32" } },
      { display: "Array", resolved: true, dataType: { kind: "Array" } },
      { display: "Float64", resolved: true, dataType: { kind: "Float64", extra: true } },
    ];

    for (const resolvedType of malformedResolvedTypes) {
      expect(() =>
        parseGraphProjectionReplacementDto({
          graphPath,
          projection: projectionWithResolvedType(resolvedType),
        }),
      ).toThrow("projection replacement");
    }
  });

  it("accepts structured dataType independently from its display label", () => {
    const replacement = {
      graphPath,
      projection: projectionWithResolvedType({
        display: "Not parsed by the frontend",
        resolved: true,
        dataType: { kind: "DataSeries", inner: { kind: "Float64" } },
      }),
    };

    expect(parseGraphProjectionReplacementDto(replacement)).toEqual(replacement);
  });

  it("strictly branches event and function projection replacement wire shapes", () => {
    const eventReplacement = { graphPath, projection: projection(graphPath, 5) };
    const functionReplacement = {
      graphPath: functionPath,
      projection: projection(functionPath, 5),
      functionEditorProjection: functionEditorProjection(),
    };

    expect(parseGraphProjectionReplacementDto(eventReplacement)).toEqual(eventReplacement);
    expect(parseGraphProjectionReplacementDto(functionReplacement)).toEqual(functionReplacement);
    expect(() =>
      parseGraphProjectionReplacementDto({
        ...eventReplacement,
        functionEditorProjection: functionEditorProjection(),
      }),
    ).toThrow("projection replacement");
    expect(() =>
      parseGraphProjectionReplacementDto({
        graphPath: functionPath,
        projection: projection(functionPath, 5),
      }),
    ).toThrow("projection replacement");
  });

  it.each(["events/Sales Report 中文.yssbi-event", "functions/销售 预测.yssbi-function"])(
    "parses opaque replacement path %j",
    (path) => {
      const replacement = path.startsWith("functions/")
        ? {
            graphPath: path,
            projection: projection(path, 5),
            functionEditorProjection: functionEditorProjection(),
          }
        : { graphPath: path, projection: projection(path, 5) };
      expect(parseGraphProjectionReplacementDto(replacement)).toEqual(replacement);
    },
  );

  it("rejects empty and whitespace-only Struct keys in function replacement pins", () => {
    for (const inner of ["", "   "]) {
      const malformed = {
        graphPath: functionPath,
        projection: projection(functionPath, 5),
        functionEditorProjection: {
          ...functionEditorProjection(),
          outputs: [{ id: "return", name: "Model", dataType: { kind: "Struct", inner } }],
        },
      };
      expect(() => parseGraphProjectionReplacementDto(malformed)).toThrow("projection replacement");
    }
  });
});
