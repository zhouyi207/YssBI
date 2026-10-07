import { describe, expect, it } from "vitest";
import type {
  EditorConnectionProjectionDto,
  PortAddressDto,
} from "@/shared/types/dto/editorProjection";
import type { GraphEntityBucket } from "@/features/core/dataStore/graphEntityAccess";
import { portAddressKey } from "@/features/domain/editorProjection";
import { hasPinViewTarget, inspectableRefsFromPinView } from "./pinViewTarget";

const graphPath = "events/Main.yssbi-event";
const output: PortAddressDto = {
  kind: "declared",
  nodeId: "node-out",
  portKey: "result",
};
const input: PortAddressDto = {
  kind: "declared",
  nodeId: "node-in",
  portKey: "data",
};
const connection: EditorConnectionProjectionDto = {
  connectionId: "connection-1",
  output,
  input,
  order: null,
};

function indexConnections(
  connections: EditorConnectionProjectionDto[],
): Pick<GraphEntityBucket, "connections" | "pinConnections"> {
  const graph: Pick<GraphEntityBucket, "connections" | "pinConnections"> = {
    connections: {},
    pinConnections: {},
  };
  for (const { connectionId: id, input, output, order } of connections) {
    const from = portAddressKey(output);
    const to = portAddressKey(input);
    graph.connections[id] = { id, input, output, order, from, to };
    (graph.pinConnections[from] ??= []).push(id);
    (graph.pinConnections[to] ??= []).push(id);
  }
  return graph;
}

describe("pinViewTarget", () => {
  it("builds an authoritative output-pin result ref before any run", () => {
    const target = {
      graphPath,
      address: output,
      direction: "output" as const,
    };

    expect(hasPinViewTarget(target, undefined)).toBe(true);
    expect(inspectableRefsFromPinView(target, undefined)).toEqual([
      { kind: "outputPin", graphPath, output },
    ]);
  });

  it("resolves an input only to its connected upstream output address", () => {
    const graph = indexConnections([connection]);
    const target = {
      graphPath,
      address: input,
      direction: "input" as const,
    };

    expect(hasPinViewTarget(target, graph)).toBe(true);
    expect(inspectableRefsFromPinView(target, graph)).toEqual([
      { kind: "outputPin", graphPath, output },
    ]);
  });

  it("never creates input result when the connection does not target that input", () => {
    const graph = indexConnections([connection]);
    // A diagnostic-blocked, wrong-direction edge may reference an input as its source.
    const target = { graphPath, address: output, direction: "input" as const };
    expect(hasPinViewTarget(target, graph)).toBe(false);
    expect(inspectableRefsFromPinView(target, graph)).toEqual([]);
  });

  it("has no result reference for unconnected input pins", () => {
    const target = { graphPath, address: input, direction: "input" as const };
    for (const graph of [undefined, indexConnections([])]) {
      expect(hasPinViewTarget(target, graph)).toBe(false);
      expect(inspectableRefsFromPinView(target, graph)).toEqual([]);
    }
  });

  it("preserves indexed order and duplicate endpoints while isolating repeatable ports", () => {
    const address: PortAddressDto = {
      kind: "instance",
      nodeId: input.nodeId,
      templateKey: "values",
      instanceId: "first",
    };
    const neighbor: PortAddressDto = { ...address, instanceId: "second" };
    const otherOutput: PortAddressDto = { ...output, portKey: "other" };
    const graph = indexConnections([
      { ...connection, connectionId: "second", input: address, output: otherOutput },
      { ...connection, connectionId: "reverse", input: neighbor, output: address },
      { ...connection, input: address },
      { ...connection, connectionId: "self", input: address, output: address },
    ]);
    const target = { graphPath, address, direction: "input" as const };

    expect(hasPinViewTarget(target, graph)).toBe(true);
    expect(inspectableRefsFromPinView(target, graph)).toEqual(
      [otherOutput, output, address, address].map((output) => ({
        kind: "outputPin",
        graphPath,
        output,
      })),
    );
    expect(inspectableRefsFromPinView({ ...target, address: neighbor }, graph)).toEqual([
      { kind: "outputPin", graphPath, output: address },
    ]);
  });
});
