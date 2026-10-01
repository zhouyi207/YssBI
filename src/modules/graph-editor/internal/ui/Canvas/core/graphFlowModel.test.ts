import { describe, expect, it } from "vitest";
import { makeGraphFlowFixture } from "@/tests/helpers/graphFlowFixture";
import { createGraphFlowModelProjector, resolveFlowPinAction } from "./graphFlowModel";

describe("graph flow projection adapter", () => {
  it("preserves node, connection and port identities while reflecting dynamic row order", () => {
    const bucket = makeGraphFlowFixture();
    const project = createGraphFlowModelProjector();
    const first = project(bucket);
    expect(first.edges[0]).toMatchObject({
      id: "original",
      source: "source",
      target: "target",
      sourceHandle: "out",
      targetHandle: "left",
    });
    expect(first.nodes.find((node) => node.id === "managed")).toMatchObject({
      draggable: false,
      selectable: false,
    });
    const reorderedBucket = {
      ...bucket,
      nodes: {
        ...bucket.nodes,
        target: { ...bucket.nodes.target, pinIds: [...bucket.nodes.target.pinIds].reverse() },
      },
    };
    const reordered = project(reorderedBucket);
    expect(reordered.nodes[0]).toBe(first.nodes[0]);
    expect(reordered.edges).toBe(first.edges);
    expect(reordered.nodes[1].data.handlesKey).not.toBe(first.nodes[1].data.handlesKey);
    expect(reordered.edges[0]).toEqual(first.edges[0]);
    const unchanged = project({ ...reorderedBucket });
    expect(unchanged.nodes).toBe(reordered.nodes);
    expect(unchanged.edges).toBe(reordered.edges);
    expect(unchanged.nodeIds).toBe(reordered.nodeIds);
    reordered.nodes[0].position.x = 100;
    expect(reordered.pins.out).toBe(bucket.pins.out);
    expect(bucket.nodes.source.position.x).toBe(0);
    expect(bucket.pins.out.name).toBe("out");
  });

  it("maps modifier keys to projected pin capabilities", () => {
    const bucket = makeGraphFlowFixture();
    const model = createGraphFlowModelProjector()(bucket);
    const event = { button: 0, altKey: false, ctrlKey: false, metaKey: false };
    expect(resolveFlowPinAction({ ...event, ctrlKey: true }, model.pins.left)).toBe(
      "moveConnections",
    );
    expect(resolveFlowPinAction({ ...event, altKey: true }, model.pins.left)).toBe("disconnect");
    expect(resolveFlowPinAction({ ...event, ctrlKey: true }, model.pins.right)).toBe("none");
  });
});
