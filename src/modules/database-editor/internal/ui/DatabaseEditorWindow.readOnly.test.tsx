// @vitest-environment happy-dom
import { act, createElement } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createRoot, type Root } from "react-dom/client";
import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";
import { TooltipProvider } from "@/components/ui/tooltip";

const mocks = vi.hoisted(() => ({
  initProjectSync: vi.fn(),
  useProjectSync: vi.fn(),
  usePersistedWindow: vi.fn(),
  useCurrentWindowActions: vi.fn(() => ({
    close: vi.fn(),
    maximize: vi.fn(),
    minimize: vi.fn(),
    show: vi.fn(),
  })),
  useWindowMaximized: vi.fn(() => false),
  useCustomTitleBar: vi.fn(() => false),
  useDataLoader: vi.fn(),
  useSelection: vi.fn(),
  useDatabaseEditorKeyboard: vi.fn(),
  getGridSelectionPrimaryCellText: vi.fn(() => ""),
  useDatabaseExport: vi.fn(() => vi.fn()),
}));

vi.mock("@/features/application/initialization", () => ({
  useProjectSync: mocks.useProjectSync,
}));

vi.mock("@/features/application/window", () => ({
  useCurrentWindowActions: mocks.useCurrentWindowActions,
  usePersistedWindow: mocks.usePersistedWindow,
  useWindowMaximized: mocks.useWindowMaximized,
}));

vi.mock("@/features/application/window/useWindowDecorations", () => ({
  useCustomTitleBar: mocks.useCustomTitleBar,
}));

vi.mock("@/features/application/databaseEditor", () => ({
  getGridSelectionPrimaryCellText: mocks.getGridSelectionPrimaryCellText,
  useDatabaseEditorKeyboard: mocks.useDatabaseEditorKeyboard,
  useDatabaseExport: mocks.useDatabaseExport,
  useDataLoader: mocks.useDataLoader,
  useSelection: mocks.useSelection,
}));

vi.mock("./Table", () => ({
  DataTable: () => createElement("div", { "data-testid": "read-only-table" }),
}));

vi.mock("@/features/core/dataStore", async () => {
  const actual = await vi.importActual<typeof import("@/features/core/dataStore")>(
    "@/features/core/dataStore",
  );
  return { ...actual, initProjectSync: mocks.initProjectSync };
});

import { DatabaseEditorWindow } from "./DatabaseEditorWindow";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const database = {
  id: "sales",
  name: "Sales",
  columns: [{ name: "amount", type: "Int64" }],
  rowCount: 1,
  columnCount: 1,
};

describe("DatabaseEditorWindow read-only composition", () => {
  let root: Root;
  let host: HTMLDivElement;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.initProjectSync.mockResolvedValue(undefined);
    mocks.useDataLoader.mockReturnValue({
      CHUNK_SIZE: 100,
      goToNextPage: vi.fn(),
      goToPreviousPage: vi.fn(),
      lastFetchMs: null,
      loadedRowIds: [1],
      loadedRows: [[1]],
      loading: false,
      loadInitialRows: vi.fn(),
      pageIndex: 0,
      pageSize: 100,
      pageStartIndex: 0,
      refreshData: vi.fn(),
      setLoadedRows: vi.fn(),
      totalPages: 1,
    });
    mocks.useSelection.mockReturnValue({
      clearSelection: vi.fn(),
      selectAll: vi.fn(),
      selection: null,
      setSelection: vi.fn(),
    });
    useDatabaseStore.setState({ databases: { sales: database }, revisions: { sales: 1 } });
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() =>
      root.render(createElement(TooltipProvider, null, createElement(DatabaseEditorWindow))),
    );
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("keeps inspection, paging, and export controls without mutation controls", () => {
    expect(host.querySelector('[data-testid="read-only-table"]')).not.toBeNull();
    const buttonLabels = Array.from(host.querySelectorAll("button"))
      .map((button) => `${button.textContent} ${button.getAttribute("aria-label") ?? ""}`)
      .join(" ")
      .toLowerCase();

    expect(buttonLabels).not.toMatch(/save|undo|redo|insert|delete|保存|撤销|重做|插入|删除/);
    expect(mocks.useDatabaseExport).toHaveBeenCalled();
    expect(mocks.useDatabaseEditorKeyboard).toHaveBeenCalledWith(
      expect.objectContaining({
        clearSelection: expect.any(Function),
        selectAll: expect.any(Function),
      }),
    );
  });
});
