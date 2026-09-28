import { resourceKey } from "@/features/core/resource";
import { afterEach, expect, it, vi } from "vitest";
import { buildFileResourceMeta, useResourceStore } from "@/features/core/resource";
import {
  startProjectLifecycle,
  clearProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { pruneEditorPanelsForMissingResources } from "./pruneEditorPanels";

const mocks = vi.hoisted(() => ({
  beforeCommit: () => {},
  release: vi.fn(),
  remove: vi.fn(async (_tokens: unknown, isCurrent: () => boolean) => {
    mocks.beforeCommit();
    return isCurrent() ? ("committed" as const) : ("stale" as const);
  }),
}));
vi.mock("@/modules/workbench/public", () => ({
  commitWorkbenchPanelRemoval: mocks.remove,
  releaseEditorPaneState: mocks.release,
  workbenchLayoutRead: {
    listPanels: () => [
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
    ],
  },
}));
afterEach(() => {
  clearProjectLifecycle();
  useResourceStore.getState().clear();
  vi.clearAllMocks();
  mocks.beforeCommit = () => {};
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
  useResourceStore
    .getState()
    .patchResource({ id: path, kind: "chart" }, { hasDirtyDocument: false });
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
  mocks.beforeCommit = () =>
    useResourceStore.getState().patchResource({ id: path, kind: "chart" }, { exists: true });
  await pruneEditorPanelsForMissingResources();
  expect(mocks.release).not.toHaveBeenCalled();
});
