import { afterEach, describe, expect, it } from "vitest";
import { useGraphInteractionStore } from "@/features/core/graphInteraction/graphInteractionStore";
import { makeProjectedPinData } from "@/tests/helpers/editorProjectionFixtures";
import { getConnectPreview } from "./connectPreview";

const sourcePin = makeProjectedPinData({
  id: "source",
  nodeId: "node-a",
  name: "Source",
  direction: "output",
  dataType: { kind: "Float64" },
});

function start(graphPath: string, groupId: string, worldX: number, connectionId: string) {
  useGraphInteractionStore.getState().startInteraction(graphPath, {
    type: "drawingConnection",
    session: {
      groupId,
      pointerId: 0,
      graphPath,
      source: sourcePin,
      screenX: 0,
      screenY: 0,
      worldX,
      worldY: worldX,
      hoveredTarget: null,
      snappedTarget: null,
      snappedWorld: null,
      feedback: { kind: "replace", displacedConnectionIds: [connectionId] },
    },
  });
}

afterEach(() => useGraphInteractionStore.setState({ interactions: {}, positionOverrides: {} }));

describe("connectPreview scoped projection", () => {
  it("reads only the exact graphPath and groupId with concurrent interactions", () => {
    start("events/one", "group-a", 10, "connection-a");
    start("events/two", "group-b", 20, "connection-b");

    expect(getConnectPreview({ graphPath: "events/one", groupId: "group-a" })).toMatchObject({
      active: true,
      worldX: 10,
      groupId: "group-a",
      highlightedConnectionIds: ["connection-a"],
    });
    expect(getConnectPreview({ graphPath: "events/two", groupId: "group-b" })).toMatchObject({
      active: true,
      worldX: 20,
      groupId: "group-b",
      highlightedConnectionIds: ["connection-b"],
    });
    expect(getConnectPreview({ graphPath: "events/one", groupId: "group-b" })).toMatchObject({
      active: false,
    });
  });

  it("replaces the visual owner when another pane starts interaction for the same graph", () => {
    start("events/shared", "group-a", 10, "connection-a");
    start("events/shared", "group-b", 20, "connection-b");

    expect(getConnectPreview({ graphPath: "events/shared", groupId: "group-a" }).active).toBe(
      false,
    );
    expect(getConnectPreview({ graphPath: "events/shared", groupId: "group-b" })).toMatchObject({
      active: true,
      worldX: 20,
      highlightedConnectionIds: ["connection-b"],
    });
  });
});
