import { afterEach, expect, it, vi } from "vitest";
import { buildGraphResourceMeta, useResourceStore } from "@/features/core/resource";
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
  workbenchDockviewRead: {
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
          resourceKind: "event",
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

it("closes retained missing resources but rejects a queued close after the resource reappears", async () => {
  startProjectLifecycle("project-a");
  const path = "charts/Chart.yssbi-chart";
  useResourceStore.getState().setSnapshot({
    resources: [
      buildGraphResourceMeta("event", "events/Event.yssbi-event", "Event"),
      {
        id: path,
        kind: "chart",
        name: "Chart",
        uri: `yssbi://chart/${path}`,
        exists: false,
        loaded: true,
        hasDirtyDocument: true,
        hasStaleDocument: false,
        hasConflictDocument: true,
      },
    ],
  });
  await pruneEditorPanelsForMissingResources();
  expect(mocks.remove).toHaveBeenCalledWith(
    [expect.objectContaining({ panelInstanceId: "missing" })],
    expect.any(Function),
  );
  expect(mocks.release).toHaveBeenCalledExactlyOnceWith("missing");
  expect(useResourceStore.getState().resources[`yssbi://chart/${path}`].hasDirtyDocument).toBe(
    true,
  );
  mocks.release.mockClear();
  mocks.beforeCommit = () =>
    useResourceStore.getState().patchResource({ id: path, kind: "chart" }, { exists: true });
  await pruneEditorPanelsForMissingResources();
  expect(mocks.release).not.toHaveBeenCalled();
});
