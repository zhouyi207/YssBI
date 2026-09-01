// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkbenchPanelInfo } from "@/features/core/dockview/workbenchRead";
import { useOpenWorksheet } from "./useWorksheetManagement";

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
    "worksheets/Summary.yssbi-worksheet": { revision: 1 },
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
  PROJECT_TREE_CATEGORY_IDS: { worksheets: "worksheets" },
  useSidebarStore: {
    getState: () => ({
      setProjectTreeCategoryExpanded: mocks.setProjectTreeCategoryExpanded,
    }),
  },
}));

vi.mock("@/features/core/worksheet/worksheetStore", () => ({
  useWorksheetStore: {
    getState: () => ({
      documents: mocks.documents,
      upsertDocument: vi.fn(),
    }),
    setState: vi.fn(),
  },
}));

vi.mock("@/services/worksheet/worksheetService", () => ({
  WorksheetService: { loadWorksheet: vi.fn() },
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
  panelInstanceId: "worksheet-panel",
  groupId: "editor-group",
  component: "WorksheetEditor",
  title: "Summary",
  metadata: {
    role: "editor",
    resourceRef: "worksheets/Summary.yssbi-worksheet",
    resourceKind: "worksheet",
    pinned: true,
  },
  active: true,
  location: { type: "grid" },
};

let openWorksheet: ReturnType<typeof useOpenWorksheet>;

function Harness(): null {
  openWorksheet = useOpenWorksheet();
  return null;
}

describe("useOpenWorksheet", () => {
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

  it("awaits the editor open before revealing the worksheet in the project tree", async () => {
    const deferred = createDeferred<WorkbenchPanelInfo>();
    mocks.openEditorPanel.mockReturnValueOnce(deferred.promise);

    let opening!: Promise<void>;
    await act(async () => {
      opening = openWorksheet("worksheets/Summary.yssbi-worksheet", "Summary");
      await Promise.resolve();
    });

    expect(mocks.setProjectTreeCategoryExpanded).not.toHaveBeenCalled();

    deferred.resolve(openedPanel);
    await act(async () => opening);

    expect(mocks.revealWorkbenchView).toHaveBeenCalledWith("project");
    expect(mocks.setProjectTreeCategoryExpanded).toHaveBeenCalledWith("worksheets", true);
  });

  it("contains an editor-open rejection whose feedback was already presented", async () => {
    const handled = new Error("layout feedback already presented");
    mocks.handledRejection = handled;
    mocks.openEditorPanel.mockRejectedValueOnce(handled);

    await expect(
      openWorksheet("worksheets/Summary.yssbi-worksheet", "Summary"),
    ).resolves.toBeUndefined();

    expect(mocks.setProjectTreeCategoryExpanded).not.toHaveBeenCalled();
  });
});
