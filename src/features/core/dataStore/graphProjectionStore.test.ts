import { beforeEach, describe, expect, it, vi } from "vitest";
import type { EditorGraphProjectionDto, PortAddressDto } from "@/shared/types/dto/editorProjection";
import { portAddressKey } from "@/features/domain/editorProjection";
import {
  getGraphDiagnostics,
  getGraphProjectionBasis,
  getGraphRequestGeneration,
  getGraphSourceRevision,
  hasGraphBlockingDiagnostics,
} from "./graphEntityAccess";
import { useGraphProjectionStore } from "./graphProjectionStore";
import { toUiNode } from "./nodeView";

const output: PortAddressDto = {
  kind: "declared",
  nodeId: "shared-node",
  portKey: "output",
};
const input: PortAddressDto = {
  kind: "instance",
  nodeId: "shared-node",
  templateKey: "input",
  instanceId: "input-1",
};

function projection(
  graphPath = "functions/main",
  sourceRevision = 4,
  title = "Localized title",
): EditorGraphProjectionDto {
  const graphOutput = { ...output, nodeId: "shared-node" };
  const graphInput = { ...input, nodeId: "shared-node" };
  return {
    basis: {
      graphPath,
      registryFingerprint: "0102030000000000000000000000000000000000000000000000000000000000",
      resourceVersions: {},
    },
    graphPath,
    sourceRevision,
    nodes: [
      {
        graphPath,
        nodeId: "shared-node",
        nodeTypeId: "unknown.projected-node",
        position: { x: 10, y: 20 },
        display: {
          title,
          userLabel: null,
          iconId: "projected-icon",
          styleId: "projected-style",
        },
        ports: [
          {
            address: graphOutput,
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
            resolvedType: { display: "Number", resolved: true, dataType: { kind: "Float64" } },
            resolvedSchema: null,
            status: "resolved",
          },
          {
            address: graphInput,
            display: { label: "Input", instanceLabel: "Input 1" },
            direction: "input",
            kind: "data",
            orphan: false,
            canRemove: true,
            connections: {
              current: 1,
              maximum: 1,
              ordered: false,
              canAppend: false,
              canReplace: true,
              canMove: true,
            },
            input: {
              literalOverride: 2,
              protocolDefault: 1,
              effective: "connections",
            },
            resolvedType: { display: "Number", resolved: true, dataType: { kind: "Float64" } },
            resolvedSchema: null,
            status: "resolved",
          },
        ],
        portInstanceAdditions: [
          {
            templateKey: "input",
            label: "Input",
            direction: "input",
            canAdd: true,
          },
        ],
        parameterEditors: [
          {
            key: "factor",
            display: { title: "Factor", description: null },
            editor: "number",
            presentation: "detailPanel",
            valueType: { kind: "Float64" },
            multiline: false,
            value: 2,
            configuration: null,
            inheritedValue: null,
            valueSource: null,
            options: null,
          },
        ],
        capabilities: {
          managed: false,
          canCopy: true,
          canDelete: true,
          canEditLabel: true,
          canEditParameters: true,
          supportsInlineLiterals: true,
        },
        diagnostics: [],
      },
    ],
    connections: [
      {
        connectionId: "connection-1",
        output: graphOutput,
        input: graphInput,
        order: null,
      },
    ],
    diagnostics: [
      {
        code: "graph.info",
        message: "Projected diagnostic",
        severity: "information",
        blocking: false,
        location: { kind: "graph" },
        related: [],
      },
    ],
    outcome: { type: "success" },
    hasBlockingDiagnostics: false,
  };
}

describe("graphProjectionStore projection replacement", () => {
  it("requires projection metadata on every graph bucket", () => {
    expect(getGraphProjectionBasis({ graphEntities: {} }, "missing")).toBeUndefined();
    expect(getGraphSourceRevision({ graphEntities: {} }, "missing")).toBeUndefined();
    expect(getGraphRequestGeneration({ graphEntities: {} }, "missing")).toBeUndefined();
    expect(getGraphDiagnostics({ graphEntities: {} }, "missing")).toBeUndefined();
    expect(hasGraphBlockingDiagnostics({ graphEntities: {} }, "missing")).toBeUndefined();
  });

  it("keeps projected canvas nodes independent from registry metadata", () => {
    const store = useGraphProjectionStore.getState();
    store.replaceProjection("functions/main", projection(), 1);

    const bucket = useGraphProjectionStore.getState().graphEntities["functions/main"];
    const canvasNode = toUiNode(bucket.nodes["shared-node"], {
      pins: bucket.nodes["shared-node"].pinIds.map((key) => ({
        pin: bucket.pins[key],
        connectionIds: bucket.pinConnections[key],
      })),
    });

    expect(canvasNode.position).toEqual({ x: 10, y: 20 });
    expect(canvasNode.title).toBe("Localized title");
  });

  it("constructs the candidate bucket before entering the Zustand setter", () => {
    const store = useGraphProjectionStore.getState();
    store.replaceProjection("functions/main", projection(), 1);
    const previous = useGraphProjectionStore.getState().graphEntities["functions/main"];
    const malformed = projection("functions/main", 5);
    Object.defineProperty(malformed.nodes[0].ports[0], "kind", {
      get: () => {
        throw new Error("candidate conversion failed");
      },
    });

    const result = store.replaceProjection("functions/main", malformed, 2);
    expect(result).toMatchObject({
      applied: false,
      reason: "invalid",
      error: expect.any(Error),
    });
    expect((result as { error?: Error }).error?.message).toBe("candidate conversion failed");
    expect(useGraphProjectionStore.getState().graphEntities["functions/main"]).toBe(previous);
  });
  beforeEach(() => {
    useGraphProjectionStore.setState({ graphEntities: {} });
  });

  it("atomically replaces a graph with projected canvas entities and metadata", () => {
    const nextProjection = projection();

    const result = useGraphProjectionStore
      .getState()
      .replaceProjection("functions/main", nextProjection, 1);

    const bucket = useGraphProjectionStore.getState().graphEntities["functions/main"];
    expect(result).toEqual({ applied: true, reason: "newer" });
    expect(bucket.sourceRevision).toBe(4);
    expect(bucket.requestGeneration).toBe(1);
    expect(bucket.nodes["shared-node"]).toMatchObject({
      nodeType: "unknown.projected-node",
      display: {
        title: "Localized title",
        styleId: "projected-style",
        iconId: "projected-icon",
      },
    });
    expect(bucket.pins[portAddressKey(input)]).toMatchObject({
      id: portAddressKey(input),
      address: input,
      canRemove: true,
    });
    expect(bucket.connections["connection-1"].from).toBe(portAddressKey(output));
    expect(bucket.diagnostics).toEqual(nextProjection.diagnostics);
    const state = useGraphProjectionStore.getState();
    expect(getGraphProjectionBasis(state, "functions/main")).toEqual(nextProjection.basis);
    expect(getGraphSourceRevision(state, "functions/main")).toBe(4);
    expect(getGraphRequestGeneration(state, "functions/main")).toBe(1);
    expect(getGraphDiagnostics(state, "functions/main")).toEqual(nextProjection.diagnostics);
    expect(hasGraphBlockingDiagnostics(state, "functions/main")).toBe(false);

    const canvasNode = toUiNode(bucket.nodes["shared-node"], {
      pins: bucket.nodes["shared-node"].pinIds.map((key) => ({
        pin: bucket.pins[key],
        connectionIds: bucket.pinConnections[key],
      })),
    });
    expect(canvasNode).toMatchObject({
      nodeType: "unknown.projected-node",
      title: "Localized title",
      styleId: "projected-style",
    });
  });

  it("ignores a lower source revision even from a newer request generation", () => {
    const store = useGraphProjectionStore.getState();
    store.replaceProjection("functions/main", projection("functions/main", 4), 2);
    const previous = useGraphProjectionStore.getState().graphEntities["functions/main"];

    const result = store.replaceProjection("functions/main", projection("functions/main", 3), 3);

    expect(result.applied).toBe(false);
    expect(useGraphProjectionStore.getState().graphEntities["functions/main"]).toBe(previous);
  });

  it("ignores older request generations even when their revision is higher", () => {
    const store = useGraphProjectionStore.getState();
    store.replaceProjection("functions/main", projection("functions/main", 4), 2);
    const previous = useGraphProjectionStore.getState().graphEntities["functions/main"];

    const result = store.replaceProjection("functions/main", projection("functions/main", 5), 1);

    expect(result.applied).toBe(false);
    expect(useGraphProjectionStore.getState().graphEntities["functions/main"]).toBe(previous);
  });

  it("allows a newer generation to replace same-revision localized display data", () => {
    const store = useGraphProjectionStore.getState();
    store.replaceProjection("functions/main", projection("functions/main", 4, "English"), 1);

    const result = store.replaceProjection(
      "functions/main",
      projection("functions/main", 4, "本地化标题"),
      2,
    );

    expect(result).toEqual({ applied: true, reason: "newer" });
    expect(
      useGraphProjectionStore.getState().graphEntities["functions/main"].nodes["shared-node"]
        .display.title,
    ).toBe("本地化标题");
  });

  it("leaves the previous bucket byte-for-byte unchanged for malformed projections", () => {
    const store = useGraphProjectionStore.getState();
    store.replaceProjection("functions/main", projection(), 1);
    const previous = useGraphProjectionStore.getState().graphEntities["functions/main"];
    const previousBytes = JSON.stringify(previous);
    const malformed = projection("functions/main", 5);
    malformed.connections[0].input = {
      kind: "declared",
      nodeId: "missing-node",
      portKey: "missing-port",
    };

    const result = store.replaceProjection("functions/main", malformed, 2);

    expect(result.applied).toBe(false);
    expect(useGraphProjectionStore.getState().graphEntities["functions/main"]).toBe(previous);
    expect(JSON.stringify(previous)).toBe(previousBytes);
  });

  it("isolates overlapping projected node ids by graphPath", () => {
    const store = useGraphProjectionStore.getState();
    store.replaceProjection("functions/first", projection("functions/first", 1, "First"), 1);
    store.replaceProjection("functions/second", projection("functions/second", 1, "Second"), 1);

    expect(store.getGraphNode("functions/first", "shared-node")?.display.title).toBe("First");
    expect(store.getGraphNode("functions/second", "shared-node")?.display.title).toBe("Second");
  });

  it("installs two valid projection replacements in one store update", () => {
    const firstPath = "functions/first";
    const secondPath = "functions/second";
    const updates = vi.fn();
    const unsubscribe = useGraphProjectionStore.subscribe(updates);

    const result = useGraphProjectionStore.getState().replaceProjectionsAtomically([
      { graphPath: firstPath, projection: projection(firstPath, 1, "First") },
      { graphPath: secondPath, projection: projection(secondPath, 1, "Second") },
    ]);

    unsubscribe();
    expect(result).toEqual({ applied: true, graphPaths: [firstPath, secondPath] });
    expect(updates).toHaveBeenCalledTimes(1);
    expect(
      useGraphProjectionStore.getState().graphEntities[firstPath].nodes["shared-node"].display
        .title,
    ).toBe("First");
    expect(
      useGraphProjectionStore.getState().graphEntities[secondPath].nodes["shared-node"].display
        .title,
    ).toBe("Second");
  });

  it("installs zero projection replacements when one candidate is malformed", () => {
    const firstPath = "functions/first";
    const secondPath = "functions/second";
    const store = useGraphProjectionStore.getState();
    store.replaceProjection(firstPath, projection(firstPath, 1, "Current first"), 1);
    store.replaceProjection(secondPath, projection(secondPath, 1, "Current second"), 1);
    const previousFirst = useGraphProjectionStore.getState().graphEntities[firstPath];
    const previousSecond = useGraphProjectionStore.getState().graphEntities[secondPath];
    const malformed = projection(secondPath, 2, "Malformed second");
    malformed.connections[0].input = {
      kind: "declared",
      nodeId: "missing-node",
      portKey: "missing-port",
    };
    const updates = vi.fn();
    const unsubscribe = useGraphProjectionStore.subscribe(updates);

    const result = useGraphProjectionStore.getState().replaceProjectionsAtomically([
      { graphPath: firstPath, projection: projection(firstPath, 2, "Replacement first") },
      { graphPath: secondPath, projection: malformed },
    ]);

    unsubscribe();
    expect(result).toMatchObject({ applied: false, reason: "invalid" });
    expect(updates).not.toHaveBeenCalled();
    expect(useGraphProjectionStore.getState().graphEntities[firstPath]).toBe(previousFirst);
    expect(useGraphProjectionStore.getState().graphEntities[secondPath]).toBe(previousSecond);
  });
});
