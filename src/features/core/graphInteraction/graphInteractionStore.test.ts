import { beforeEach, describe, expect, it } from "vitest";
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
});
