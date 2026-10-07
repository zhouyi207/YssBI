import { beforeEach, describe, expect, it } from "vitest";
import { focusGraphPanelSession, deactivateGraphPanelSession } from "./graphPanelSession";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";

describe("graph focus bookkeeping", () => {
  beforeEach(() => useGraphSessionStore.getState().reset());
  it("focuses before loading and skips duplicate focus publications", () => {
    let notifications = 0;
    const unsubscribe = useGraphSessionStore.subscribe(() => notifications++);
    focusGraphPanelSession("events/A", "group-a");
    focusGraphPanelSession("events/A", "group-a");
    expect(useGraphSessionStore.getState().focusedSession).toEqual({
      groupId: "group-a",
      graphPath: "events/A",
    });
    expect(notifications).toBe(1);
    focusGraphPanelSession("events/B", "group-b");
    expect(useGraphSessionStore.getState().focusedSession?.graphPath).toBe("events/B");
    unsubscribe();
  });
});

describe("deactivateGraphPanelSession", () => {
  beforeEach(() => {
    useGraphSessionStore.getState().reset();
  });

  it("clears session when the closed tab owned the focused graph", () => {
    useGraphSessionStore.getState().setFocusedSession("editor", "g1");

    deactivateGraphPanelSession("editor", "g1");

    expect(useGraphSessionStore.getState().focusedSession?.graphPath ?? null).toBeNull();
  });

  it("keeps session when a background tab is closed", () => {
    useGraphSessionStore.getState().setFocusedSession("editor", "g1");

    deactivateGraphPanelSession("editor", "g2");

    expect(useGraphSessionStore.getState().focusedSession?.graphPath ?? null).toBe("g1");
  });
});
