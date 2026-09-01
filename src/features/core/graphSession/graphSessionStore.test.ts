import { beforeEach, describe, expect, it } from "vitest";
import { useGraphSessionStore } from "./graphSessionStore";

describe("graphSessionStore", () => {
  beforeEach(() => {
    useGraphSessionStore.getState().reset();
  });

  it("tracks a single focused graph session", () => {
    const store = useGraphSessionStore.getState();
    expect(store.setFocusedSession("editor-a", "events/A.yssbi-event")).toBeNull();
    expect(store.setFocusedSession("editor-a", "events/B.yssbi-event")).toBe(
      "events/A.yssbi-event",
    );
    expect(store.getFocusedGraphPath()).toBe("events/B.yssbi-event");
    expect(store.getFocusedGroupId()).toBe("editor-a");
    expect(store.isFocusedGraphPath("events/B.yssbi-event")).toBe(true);
    expect(store.isFocusedGraphPath("events/A.yssbi-event")).toBe(false);
  });

  it("clears focused session only for the matching group", () => {
    const store = useGraphSessionStore.getState();
    store.setFocusedSession("editor-a", "events/A.yssbi-event");
    store.clearFocusedSession("editor-b");
    expect(store.getFocusedGraphPath()).toBe("events/A.yssbi-event");
    store.clearFocusedSession("editor-a");
    expect(store.getFocusedGraphPath()).toBeNull();
  });

  it("remaps focused graph path on rename", () => {
    const store = useGraphSessionStore.getState();
    store.setFocusedSession("editor-a", "events/Old.yssbi-event");
    store.remapFocusedGraphPath("events/Old.yssbi-event", "events/New.yssbi-event");
    expect(store.getFocusedGraphPath()).toBe("events/New.yssbi-event");
  });
});
