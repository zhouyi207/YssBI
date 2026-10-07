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

  it("isolates database previews without changing other selections", () => {
    const store = useEditorPaneStateStore;
    const actions = store.getState();
    const view = {
      databaseId: "sales",
      selectedRowNumber: 1,
      selectedColumnName: "amount",
      selectedCellText: "1",
    };
    actions.setDatabaseView("data-a", view);
    actions.setDatabaseView("data-b", { ...view, selectedCellText: "2" });
    actions.setSelectedNodeIds("graph-a", ["node-a"]);
    const initial = store.getState();
    actions.setDatabaseView("data-a", { ...view });
    expect(store.getState()).toBe(initial);

    actions.setDatabaseView("data-a", { ...view, selectedCellText: "3" });
    const updated = store.getState();
    expect(updated.databaseViews["data-a"].selectedCellText).toBe("3");
    expect(updated.databaseViews["data-b"]).toBe(initial.databaseViews["data-b"]);
    expect(updated.selections).toBe(initial.selections);

    const movedView = {
      ...updated.databaseViews["data-a"],
      selectedRowNumber: 2,
      selectedColumnName: "total",
    };
    actions.setDatabaseView("data-a", movedView);
    expect(store.getState().databaseViews["data-a"]).toEqual(movedView);
    expect(store.getState().databaseViews["data-b"]).toBe(initial.databaseViews["data-b"]);
    expect(store.getState().selections).toBe(initial.selections);
  });

  it("releases database previews on unmount, panel close and project reset", () => {
    const store = useEditorPaneStateStore;
    const actions = store.getState();
    const view = {
      databaseId: "sales",
      selectedRowNumber: 1,
      selectedColumnName: "amount",
      selectedCellText: "1",
    };
    actions.setDatabaseView("data-a", view);
    actions.setDatabaseView("data-b", view);
    actions.setDatabaseView("data-a", undefined);
    expect(store.getState().databaseViews["data-a"]).toBeUndefined();
    expect(store.getState().databaseViews["data-b"]).toBe(view);
    actions.setDatabaseView("data-a", view);
    actions.release("data-a");
    expect(store.getState().databaseViews["data-a"]).toBeUndefined();
    expect(store.getState().databaseViews["data-b"]).toBe(view);
    actions.reset();
    expect(store.getState().databaseViews).toEqual({});
    const empty = store.getState();
    actions.setDatabaseView("data-a", undefined);
    actions.reset();
    expect(store.getState()).toBe(empty);
  });
});
