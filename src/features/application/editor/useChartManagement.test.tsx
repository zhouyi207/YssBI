// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkbenchPanelInfo } from "@/features/core/dockview/workbenchRead";
import { useOpenChart } from "./useChartManagement";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type Deferred<T> = {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
  readonly reject: (reason: unknown) => void;
};

function createDeferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((onResolve, onReject) => {
    resolve = onResolve;
    reject = onReject;
  });
  return { promise, resolve, reject };
}

const mocks = vi.hoisted(() => ({
  openEditorPanel: vi.fn(),
  revealWorkbenchView: vi.fn(),
  setProjectTreeCategoryExpanded: vi.fn(),
  handledRejection: undefined as unknown,
  documents: {
    "charts/Summary.yssbi-chart": { revision: 1 },
  } as Record<string, unknown>,
}));

vi.mock("./openEditorPanel", () => ({
  openEditorPanel: mocks.openEditorPanel,
  isEditorOpenRejectionHandled: (error: unknown) => error === mocks.handledRejection,
}));

vi.mock("@/features/application/layout/workbenchLayoutActions", () => ({
  revealWorkbenchView: mocks.revealWorkbenchView,
}));

vi.mock("@/features/core/sidebar", () => ({
  PROJECT_TREE_CATEGORY_IDS: { charts: "charts" },
  useSidebarStore: {
    getState: () => ({
      setProjectTreeCategoryExpanded: mocks.setProjectTreeCategoryExpanded,
    }),
  },
}));

vi.mock("@/features/core/chart/chartDocumentStore", () => ({
  useChartDocumentStore: {
    getState: () => ({
      documents: mocks.documents,
      upsertDocument: vi.fn(),
    }),
    setState: vi.fn(),
  },
}));

vi.mock("@/services/chart/chartService", () => ({
  ChartService: { loadChart: vi.fn() },
}));

vi.mock("@/features/application/projectCommandContext", () => ({
  captureProjectCommandContext: vi.fn(),
}));

vi.mock("@/features/application/resource/resourceActions", () => ({
  commitFileFirstResourceIndex: vi.fn(),
}));

vi.mock("@/features/application/editorMutation/projectPublicationCoordinator", () => ({
  projectPublicationCoordinator: { submit: vi.fn() },
}));

vi.mock("./blockingErrorDialog", () => ({
  showBlockingIpcError: vi.fn(),
}));

const openedPanel: WorkbenchPanelInfo = {
  panelInstanceId: "chart-panel",
  groupId: "editor-group",
  component: "EditorResource",
  title: "Summary",
  metadata: {
    role: "editor",
    resourceRef: "charts/Summary.yssbi-chart",
    resourceKind: "chart",
    pinned: true,
  },
  active: true,
  location: { type: "grid" },
};

let openChart: ReturnType<typeof useOpenChart>;

function Harness(): null {
  openChart = useOpenChart();
  return null;
}

describe("useOpenChart", () => {
  let host: HTMLDivElement;
  let root: Root | null;

  beforeEach(async () => {
    vi.clearAllMocks();
    mocks.handledRejection = undefined;
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => root?.render(<Harness />));
  });

  afterEach(async () => {
    if (root) await act(async () => root?.unmount());
    root = null;
    host.remove();
  });

  it("awaits the editor open before revealing the chart in the project tree", async () => {
    const deferred = createDeferred<WorkbenchPanelInfo>();
    mocks.openEditorPanel.mockReturnValueOnce(deferred.promise);

    let opening!: Promise<void>;
    await act(async () => {
      opening = openChart("charts/Summary.yssbi-chart", "Summary");
      await Promise.resolve();
    });

    expect(mocks.setProjectTreeCategoryExpanded).not.toHaveBeenCalled();

    deferred.resolve(openedPanel);
    await act(async () => opening);

    expect(mocks.revealWorkbenchView).toHaveBeenCalledWith("project");
    expect(mocks.setProjectTreeCategoryExpanded).toHaveBeenCalledWith("charts", true);
  });

  it("contains an editor-open rejection whose feedback was already presented", async () => {
    const handled = new Error("layout feedback already presented");
    mocks.handledRejection = handled;
    mocks.openEditorPanel.mockRejectedValueOnce(handled);

    await expect(openChart("charts/Summary.yssbi-chart", "Summary")).resolves.toBeUndefined();

    expect(mocks.setProjectTreeCategoryExpanded).not.toHaveBeenCalled();
  });
});
