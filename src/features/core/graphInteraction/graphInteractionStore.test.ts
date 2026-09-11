import { beforeEach, describe, expect, it } from "vitest";
import { makeProjectedPinData } from "@/tests/helpers/editorProjectionFixtures";
import { getCanvasInteraction, useGraphInteractionStore } from "./graphInteractionStore";

beforeEach(() => useGraphInteractionStore.setState({ interactions: {} }));

describe("graph interaction ownership", () => {
  it("keeps cancellation isolated to the graph and physical group", () => {
    const store = useGraphInteractionStore.getState();
    store.startInteraction("one", {
      type: "draggingNodes",
      session: { groupId: "a", panelInstanceId: "first" },
    });
    store.startInteraction("two", {
      type: "selecting",
      session: { groupId: "b", panelInstanceId: "second" },
    });
    expect(store.cancelInteraction("one", "b")).toBe("idle");
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), "one", "a").type).toBe(
      "draggingNodes",
    );
    expect(store.cancelInteraction("one", "a")).toBe("draggingNodes");
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), "two", "b").type).toBe(
      "selecting",
    );
    store.clearGraphInteraction("two");
    expect(useGraphInteractionStore.getState().interactions.two).toBeUndefined();
  });

  it("captures a palette source without retaining a mutable pin projection", () => {
    const source = makeProjectedPinData({ id: "out", nodeId: "node", direction: "output" });
    useGraphInteractionStore.getState().startInteraction("one", {
      type: "pendingNodeCreation",
      session: {
        graphPath: "one",
        groupId: "a",
        panelInstanceId: "first",
        source,
        screenX: 5,
        screenY: 8,
      },
    });
    source.name = "changed";
    const captured = getCanvasInteraction(useGraphInteractionStore.getState(), "one", "a");
    expect(captured.type).toBe("pendingNodeCreation");
    if (captured.type === "pendingNodeCreation")
      expect(captured.session.source?.name).not.toBe("changed");
  });
});
