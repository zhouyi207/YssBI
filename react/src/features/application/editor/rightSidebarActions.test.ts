import { beforeEach, describe, expect, it, vi } from "vitest";
import { useEditorStore } from "@/features/core/editor";
import type { ResourceKind } from "@/shared/types/domain/resource";

const mocks = vi.hoisted(() => ({
  reveal: vi.fn(),
  selections: {} as Record<string, string[]>,
  active: {
    panelInstanceId: "graph-panel",
    metadata: {
      resourceKind: "event_graph" as ResourceKind,
      resourceRef: "events/Main.yssbi-event",
    },
  },
}));
vi.mock("@/modules/workbench/public", () => ({
  workbenchLayoutRead: { isReady: true, getActiveEditorPanel: () => mocks.active },
  getPaneSelection: (id: string) => ({
    selectedNodeIds: mocks.selections[id] ?? [],
    selectedConnectionIds: [],
  }),
  revealWorkbenchView: mocks.reveal,
}));
import {
  detailFocusForEditorResource,
  revealDetails,
  setDetailContext,
  setInspectionContext,
} from "./rightSidebarActions";

const graphScope = {
  resourceKind: "event_graph" as const,
  resourceRef: "events/Main.yssbi-event",
  panelInstanceId: "graph-panel",
};

beforeEach(() => {
  vi.clearAllMocks();
  mocks.selections = {};
  mocks.active = {
    panelInstanceId: graphScope.panelInstanceId,
    metadata: { resourceKind: graphScope.resourceKind, resourceRef: graphScope.resourceRef },
  };
  useEditorStore.setState({ detailFocus: null });
});

describe("right sidebar context actions", () => {
  it("resolves activation from each pane's selection instead of retaining another pane's node", () => {
    mocks.selections["graph-panel"] = ["node-1"];
    setInspectionContext(graphScope, ["node-1"]);
    setDetailContext(
      detailFocusForEditorResource(graphScope.resourceKind, graphScope.resourceRef, "graph-panel"),
    );
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "node",
      id: "node-1",
      graphPath: graphScope.resourceRef,
    });
    setDetailContext(
      detailFocusForEditorResource(graphScope.resourceKind, graphScope.resourceRef, "second-panel"),
    );
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "event_graph",
      path: graphScope.resourceRef,
    });
    mocks.selections["second-panel"] = ["node-2"];
    setDetailContext(
      detailFocusForEditorResource(graphScope.resourceKind, graphScope.resourceRef, "second-panel"),
    );
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "node",
      id: "node-2",
      graphPath: graphScope.resourceRef,
    });
  });

  it("publishes node context before awaiting Details reveal", async () => {
    let resolve!: () => void;
    mocks.reveal.mockImplementation(
      () =>
        new Promise<void>((done) => {
          resolve = done;
        }),
    );
    let settled = false;
    const revealing = revealDetails({
      kind: "node",
      id: "node-2",
      graphPath: graphScope.resourceRef,
    }).then(() => {
      settled = true;
    });
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "node",
      id: "node-2",
      graphPath: graphScope.resourceRef,
    });
    expect(mocks.reveal).toHaveBeenCalledWith("details");
    await Promise.resolve();
    expect(settled).toBe(false);
    resolve();
    await revealing;
    expect(settled).toBe(true);
  });

  it("preserves the active file's Details when a different pane clears its selection", () => {
    mocks.active = {
      panelInstanceId: "data-panel",
      metadata: { resourceKind: "database", resourceRef: "database-1" },
    };
    setDetailContext({ kind: "data", id: "database-1" });
    setInspectionContext(graphScope, []);
    expect(useEditorStore.getState().detailFocus).toEqual({ kind: "data", id: "database-1" });
    expect(mocks.reveal).not.toHaveBeenCalled();
  });
});
