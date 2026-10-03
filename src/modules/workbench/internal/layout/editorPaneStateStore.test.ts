import { beforeEach, describe, expect, it, vi } from "vitest";

import { getPaneSelection, useEditorPaneStateStore } from "./editorPaneStateStore";

describe("editor pane selection snapshots", () => {
  beforeEach(() => {
    useEditorPaneStateStore.getState().reset();
  });

  it("keeps the empty selection snapshot stable for an uninitialized panel", () => {
    const first = getPaneSelection("panel-a");
    const second = getPaneSelection("panel-a");

    expect(second).toBe(first);
  });

  it("publishes only changed pane state and preserves unrelated selections", () => {
    const store = useEditorPaneStateStore;
    const actions = store.getState();
    actions.setSelectedNodeIds("panel-a", ["node-a", "node-a"]);
    actions.setSelectedNodeIds("panel-b", ["node-b"]);
    actions.setNodeCollapsed("panel-a", "node-a", true);
    const initial = store.getState();
    const listener = vi.fn();
    const unsubscribe = store.subscribe(listener);
    try {
      actions.setSelectedNodeIds("panel-a", ["node-a"]);
      actions.setNodeCollapsed("panel-a", "node-a", true);
      actions.clearSelection("missing");
      actions.release("missing");
      expect(store.getState()).toBe(initial);
      expect(listener).not.toHaveBeenCalled();

      actions.setSelectedConnectionIds("panel-a", ["connection-a", "connection-a"]);
      expect(getPaneSelection("panel-a")).toEqual({
        selectedNodeIds: [],
        selectedConnectionIds: ["connection-a"],
      });
      expect(getPaneSelection("panel-b")).toBe(initial.selections["panel-b"]);
      expect(store.getState().collapsedNodeIds).toBe(initial.collapsedNodeIds);
      actions.setSelectedConnectionIds("panel-a", ["connection-a"]);
      expect(listener).toHaveBeenCalledTimes(1);

      actions.release("panel-a");
      expect(store.getState().selections["panel-a"]).toBeUndefined();
      expect(store.getState().collapsedNodeIds["panel-a"]).toBeUndefined();
      expect(listener).toHaveBeenCalledTimes(2);
      actions.release("panel-a");
      actions.reset();
      actions.reset();
      expect(listener).toHaveBeenCalledTimes(3);
    } finally {
      unsubscribe();
    }
  });
});
