// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useOpenChart } from "./useChartManagement";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  openEditorPanel: vi.fn(),
  activateEditorPanelAndSyncSession: vi.fn(async () => true),
  revealWorkbenchView: vi.fn(),
  setCategoryExpanded: vi.fn(),
  handledRejection: undefined as unknown,
  documents: {
    "charts/Summary.yssbi-chart": { revision: 1 },
  } as Record<string, unknown>,
}));

vi.mock("./openEditorPanel", () => ({
  openEditorPanel: mocks.openEditorPanel,
  isEditorOpenRejectionHandled: (error: unknown) => error === mocks.handledRejection,
}));

vi.mock("./activateEditorPanelAndSyncSession", () => ({
  activateEditorPanelAndSyncSession: mocks.activateEditorPanelAndSyncSession,
}));

vi.mock("@/modules/workbench/internal/application/workbenchLayoutActions", () => ({
  revealWorkbenchView: mocks.revealWorkbenchView,
}));

vi.mock("@/features/core/sidebar", () => ({
  PROJECT_TREE_CATEGORY_IDS: { charts: "charts" },
  useSidebarStore: {
    getState: () => ({
      setCategoryExpanded: mocks.setCategoryExpanded,
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

vi.mock("@/features/application/editorMutation/projectPublicationCoordinator", () => ({
  projectPublicationCoordinator: { submit: vi.fn() },
}));

vi.mock("./blockingErrorDialog", () => ({
  showBlockingIpcError: vi.fn(),
}));

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

  it("contains an editor-open rejection whose feedback was already presented", async () => {
    const handled = new Error("layout feedback already presented");
    mocks.handledRejection = handled;
    mocks.openEditorPanel.mockRejectedValueOnce(handled);

    await expect(openChart("charts/Summary.yssbi-chart", "Summary")).resolves.toBeUndefined();

    expect(mocks.activateEditorPanelAndSyncSession).not.toHaveBeenCalled();
    expect(mocks.setCategoryExpanded).not.toHaveBeenCalled();
  });
});
