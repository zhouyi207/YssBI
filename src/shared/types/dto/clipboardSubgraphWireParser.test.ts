import { describe, expect, it } from "vitest";
import type { EditorGraphMutationDto } from "./editorMutation";
import { parseClipboardSubgraphDto } from "./clipboardSubgraphWireParser";

const completeSnapshot = {
  schemaVersion: 1,
  nodes: [
    {
      localId: "node/0",
      creation: { kind: "static", nodeTypeId: "yssbi.constant.int64" },
      parameters: { value: 42, nested: { enabled: true } },
      userLabel: "Source",
      relativePosition: { x: 0, y: 10 },
    },
    {
      localId: "node/1",
      creation: {
        kind: "resourceBound",
        nodeTypeId: "yssbi.project.function.call",
        resourcePath: "functions/example",
        createArgs: { kind: "function" },
      },
      parameters: {},
      userLabel: null,
      relativePosition: { x: 120, y: 30 },
    },
  ],
  portBindings: [
    {
      address: {
        nodeId: "node/1",
        port: { kind: "instance", template: "inputs", localInstanceId: "port/0" },
      },
      binding: { kind: "userCreated", order: "a" },
    },
    {
      address: {
        nodeId: "node/1",
        port: { kind: "instance", template: "derived", localInstanceId: "port/1" },
      },
      binding: {
        kind: "resolved",
        origin: {
          kind: "functionParameter",
          function: "functions/example",
          parameter: "parameter-a",
        },
        order: "b",
        lastKnown: { label: "Input", valueType: { Concrete: "int64" } },
      },
    },
  ],
  inputStates: [
    {
      address: { nodeId: "node/1", port: { kind: "declared", key: "input" } },
      state: {
        literalOverride: {
          value_type: { Concrete: "core.int64" },
          value: { Integer: 42 },
        },
      },
    },
  ],
  connections: [
    {
      output: { nodeId: "node/0", port: { kind: "declared", key: "value" } },
      input: { nodeId: "node/1", port: { kind: "declared", key: "input" } },
      order: null,
    },
  ],
} as const;

function cloneSnapshot(): Record<string, unknown> {
  return structuredClone(completeSnapshot) as unknown as Record<string, unknown>;
}

describe("parseClipboardSubgraphDto", () => {
  it("accepts one complete strict version-1 camelCase fixture", () => {
    expect(parseClipboardSubgraphDto(completeSnapshot)).toEqual(completeSnapshot);
  });

  it("rejects foreign top-level and nested keys", () => {
    expect(() => parseClipboardSubgraphDto({ ...completeSnapshot, compatibility: true })).toThrow();
    const nested = cloneSnapshot();
    (nested.nodes as Array<Record<string, unknown>>)[0].compatibility = true;
    expect(() => parseClipboardSubgraphDto(nested)).toThrow();
  });

  it("rejects unsupported schema versions and non-array collections", () => {
    expect(() => parseClipboardSubgraphDto({ ...completeSnapshot, schemaVersion: 2 })).toThrow();
    expect(() => parseClipboardSubgraphDto({ ...completeSnapshot, nodes: {} })).toThrow();

    const untypedLiteral = cloneSnapshot();
    const inputState = (untypedLiteral.inputStates as Array<Record<string, unknown>>)[0];
    (inputState.state as Record<string, unknown>).literalOverride = 42;
    expect(() => parseClipboardSubgraphDto(untypedLiteral)).toThrow();
  });

  it("rejects empty clipboard-local IDs", () => {
    const value = cloneSnapshot();
    (value.nodes as Array<Record<string, unknown>>)[0].localId = "   ";
    expect(() => parseClipboardSubgraphDto(value)).toThrow();
  });

  it("rejects non-finite positions", () => {
    const value = cloneSnapshot();
    const first = (value.nodes as Array<Record<string, unknown>>)[0];
    (first.relativePosition as Record<string, unknown>).x = Number.POSITIVE_INFINITY;
    expect(() => parseClipboardSubgraphDto(value)).toThrow();
  });

  it("defines duplicateSubgraph and keeps insertSubgraph as raw snapshotJson wire data", () => {
    const duplicate: EditorGraphMutationDto = {
      type: "duplicateSubgraph",
      payload: { nodeIds: ["node-a"], offset: { x: 20, y: 20 } },
    };
    const insert: EditorGraphMutationDto = {
      type: "insertSubgraph",
      payload: { snapshotJson: JSON.stringify(completeSnapshot), anchor: { x: 1, y: 2 } },
    };

    expect(duplicate.payload).toEqual({ nodeIds: ["node-a"], offset: { x: 20, y: 20 } });
    expect(insert.payload).toEqual({
      snapshotJson: JSON.stringify(completeSnapshot),
      anchor: { x: 1, y: 2 },
    });
    expect(insert.payload).not.toHaveProperty("snapshot");
  });
});

export { completeSnapshot };
