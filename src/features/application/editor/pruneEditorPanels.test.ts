import { resourceKey, markResourceDirty } from "@/features/core/resource";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { buildFileResourceMeta, useResourceStore } from "@/features/core/resource";
import {
  startProjectLifecycle,
  clearProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { pruneEditorPanelsForMissingResources } from "./pruneEditorPanels";

const mocks = vi.hoisted(() => {
  const makePanels = () => [
    {
      panelInstanceId: "missing",
      groupId: "top",
      metadata: {
        role: "editor",
        resourceKind: "chart",
        resourceRef: "charts/Chart.yssbi-chart",
      },
    },
    {
      panelInstanceId: "kept",
      groupId: "top",
      metadata: {
        role: "editor",
        resourceKind: "event_graph",
        resourceRef: "events/Event.yssbi-event",
      },
    },
  ];
  return {
    makePanels,
    panels: makePanels(),
    beforeCommit: () => {},
    afterCommit: () => {},
    release: vi.fn(),
    remove: vi.fn(async (tokens: { panelInstanceId: string }[], isCurrent: () => boolean) => {
      mocks.beforeCommit();
      if (!isCurrent()) return "stale" as const;
      mocks.panels = mocks.panels.filter(
        (panel) => !tokens.some((token) => token.panelInstanceId === panel.panelInstanceId),
      );
      mocks.afterCommit();
      return "committed" as const;
    }),
  };
});
vi.mock("@/modules/workbench/public", () => ({
  commitWorkbenchPanelRemoval: mocks.remove,
  releaseEditorPaneState: mocks.release,
  workbenchLayoutRead: {
    listPanels: () => mocks.panels,
  },
}));
beforeEach(() => {
  mocks.panels = mocks.makePanels();
});
afterEach(() => {
  clearProjectLifecycle();
  useResourceStore.getState().clear();
  vi.clearAllMocks();
  mocks.beforeCommit = () => {};
  mocks.afterCommit = () => {};
});

it("retains dirty missing resources and rejects a queued clean close after the resource reappears", async () => {
  startProjectLifecycle("project-a");
  const path = "charts/Chart.yssbi-chart";
  useResourceStore.getState().setSnapshot({
    resources: [
      buildFileResourceMeta("event_graph", "events/Event.yssbi-event", "Event"),
      {
        id: path,
        kind: "chart",
        name: "Chart",
        uri: resourceKey({ id: path, kind: "chart" }),
        exists: false,
        loaded: true,
        hasDirtyDocument: true,
        hasStaleDocument: false,
        hasConflictDocument: true,
      },
    ],
  });
  await pruneEditorPanelsForMissingResources();
  expect(mocks.remove).not.toHaveBeenCalled();
  markResourceDirty({ id: path, kind: "chart" }, false);
  await pruneEditorPanelsForMissingResources();
  expect(mocks.remove).toHaveBeenCalledWith(
    [expect.objectContaining({ panelInstanceId: "missing" })],
    expect.any(Function),
  );
  expect(mocks.release).toHaveBeenCalledExactlyOnceWith("missing");
  expect(
    useResourceStore.getState().resources[resourceKey({ id: path, kind: "chart" })]
      .hasDirtyDocument,
  ).toBe(false);
  mocks.release.mockClear();
  mocks.panels = mocks.makePanels();
  mocks.beforeCommit = () =>
    useResourceStore.getState().patchResource({ id: path, kind: "chart" }, { exists: true });
  await pruneEditorPanelsForMissingResources();
  expect(mocks.release).not.toHaveBeenCalled();
});

it("preserves replacement pane state after prune commits and during release notifications", async () => {
  for (const phase of ["receipt-project", "receipt-panel", "release-project", "release-panel"]) {
    startProjectLifecycle("project-a");
    mocks.panels = mocks.makePanels();
    mocks.release.mockReset();
    mocks.afterCommit = () => {
      if (phase === "receipt-project") startProjectLifecycle("project-b");
      if (phase === "receipt-panel") mocks.panels.push(mocks.makePanels()[0]);
    };
    mocks.release.mockImplementationOnce(() => {
      if (phase === "release-project") startProjectLifecycle("project-b");
      if (phase === "release-panel") mocks.panels.push(mocks.makePanels()[1]);
    });
    await pruneEditorPanelsForMissingResources();
    expect
      .soft(mocks.release.mock.calls, phase)
      .toEqual(
        phase === "receipt-project" ? [] : phase === "receipt-panel" ? [["kept"]] : [["missing"]],
      );
  }
});
