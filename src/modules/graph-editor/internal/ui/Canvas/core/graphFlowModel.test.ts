import { describe, expect, it } from "vitest";
import { makeGraphFlowFixture } from "@/tests/helpers/graphFlowFixture";
import { buildGraphFlowModel, resolveFlowConnection, resolveFlowPinAction } from "./graphFlowModel";

describe("graph flow projection adapter", () => {
  it("preserves node, connection and port identities while reflecting dynamic row order", () => {
    const bucket = makeGraphFlowFixture();
    const first = buildGraphFlowModel(bucket);
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
    bucket.nodes.target.pinIds.reverse();
    const reordered = buildGraphFlowModel(bucket);
    expect(reordered.nodes[1].data.handlesKey).not.toBe(first.nodes[1].data.handlesKey);
    expect(reordered.edges[0]).toEqual(first.edges[0]);
    reordered.nodes[0].position.x = 100;
    reordered.pins.out.name = "local display";
    expect(bucket.nodes.source.position.x).toBe(0);
    expect(bucket.pins.out.name).toBe("out");
  });

  it("keeps output fan-out when adding or replacing an input connection", () => {
    const bucket = makeGraphFlowFixture();
    bucket.pinConnections.out.push("another-branch");
    const model = buildGraphFlowModel(bucket);
    expect(resolveFlowConnection(model, "out", "right", "connect")).toEqual({ kind: "append" });
    expect(resolveFlowConnection(model, "out", "left", "connect")).toEqual({
      kind: "replace",
      displacedConnectionIds: ["original"],
    });
    model.pins.right.orphan = true;
    expect(resolveFlowConnection(model, "out", "right", "connect")).toMatchObject({
      kind: "invalid",
      reason: "orphan",
    });
  });

  it("validates Ctrl moves between sibling inputs against their actual upstream peers", () => {
    const model = buildGraphFlowModel(makeGraphFlowFixture());
    expect(resolveFlowConnection(model, "left", "right", "moveConnections")).toEqual({
      kind: "append",
    });
    expect(resolveFlowConnection(model, "left", "out", "moveConnections")).toMatchObject({
      kind: "invalid",
    });
    model.pins.right.acceptedType.domain = [{ kind: "String" }];
    expect(resolveFlowConnection(model, "left", "right", "moveConnections")).toEqual({
      kind: "invalid",
      reason: "type-mismatch",
    });
    model.pins.right.acceptedType.domain = [{ kind: "Float64" }];
    model.pins.right.connections.maximum = 0;
    expect(resolveFlowConnection(model, "left", "right", "moveConnections")).toEqual({
      kind: "invalid",
      reason: "capacity",
    });
    const event = { button: 0, altKey: false, ctrlKey: false, metaKey: false };
    expect(resolveFlowPinAction({ ...event, ctrlKey: true }, model.pins.left)).toBe(
      "moveConnections",
    );
    expect(resolveFlowPinAction({ ...event, altKey: true }, model.pins.left)).toBe("disconnect");
    expect(resolveFlowPinAction({ ...event, ctrlKey: true }, model.pins.right)).toBe("none");
  });
});
