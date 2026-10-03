// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import { DatabaseService } from "@/services/database/databaseService";
import type { DatabaseRowsResult } from "@/services/database/databaseService";
import type { DatabaseMetadataResult } from "@/shared/types/dto/database";
import { logger } from "@/utils/frontendLogger";
import { useDataLoader } from "./useDataLoader";

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const replacementProjectInstanceId = "00000000-0000-0000-0000-000000000602";
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((settle, fail) => {
    resolve = settle;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const oldMeta: DatabaseMetadataResult = {
  dataRevision: "1",
  id: "sales",
  name: "Old sales",
  columns: [{ name: "amount", type: "Int64" }],
  rowCount: 1,
  columnCount: 1,
};

describe("useDataLoader project lifecycle ownership", () => {
  let root: Root;
  let host: HTMLDivElement;
  let loader!: ReturnType<typeof useDataLoader>;

  beforeEach(() => {
    vi.restoreAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useResourceStore.getState().clear();
    useResourceStore.getState().setSnapshot({
      resources: [
        {
          id: "sales",
          kind: "database",
          name: "Sales",
          revision: 1,
          uri: resourceKey({ kind: "database", id: "sales" }),
          exists: true,
          loaded: true,
          hasDirtyDocument: false,
          hasStaleDocument: false,
          hasConflictDocument: false,
        },
      ],
    });
    vi.spyOn(DatabaseService, "getDatabaseMeta").mockResolvedValue(oldMeta);
    useResourceStore.setState({
      databases: {
        sales: {
          id: "sales",
          name: "Sales",
          columns: [{ name: "amount", type: "Int64" }],
          rowCount: 1,
          columnCount: 1,
        },
      },
    });
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    function Harness() {
      loader = useDataLoader("sales");
      return null;
    }
    act(() => root.render(createElement(Harness)));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  async function replaceProject() {
    act(() => {
      projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
      useResourceStore.setState({
        databases: {
          sales: { id: "sales", name: "Replacement", columns: [], rowCount: 0, columnCount: 0 },
        },
      });
    });
    return useResourceStore.getState();
  }

  it("ignores a delayed page completion after project replacement", async () => {
    const page = deferred<DatabaseRowsResult>();
    vi.spyOn(DatabaseService, "getDatabaseRows").mockReturnValue(page.promise);

    let completion!: Promise<void>;
    act(() => {
      completion = loader.loadInitialRows("sales");
    });
    await vi.waitFor(() =>
      expect(DatabaseService.getDatabaseRows).toHaveBeenCalledWith(
        projectInstanceId,
        "sales",
        1,
        0,
        loader.pageSize,
      ),
    );

    const replacementStoreSnapshot = await replaceProject();
    await act(async () => {
      page.resolve({ rows: [[99]], rowIds: ["99"] });
      await completion;
    });

    expect(loader.loadedRows).toEqual([]);
    expect(loader.loadedRowIds).toEqual([]);
    expect(useResourceStore.getState()).toBe(replacementStoreSnapshot);
  });

  it("does not issue rows or mutate replacement state after delayed metadata resolves", async () => {
    act(() =>
      useResourceStore.setState({
        databases: {
          sales: { id: "sales", name: "Sales", columns: [], rowCount: 0, columnCount: 0 },
        },
      }),
    );
    const metadata = deferred<DatabaseMetadataResult>();
    vi.spyOn(DatabaseService, "getDatabaseMeta").mockReturnValue(metadata.promise);
    const rows = vi.spyOn(DatabaseService, "getDatabaseRows");

    const completion = loader.loadInitialRows("sales");
    await vi.waitFor(() =>
      expect(DatabaseService.getDatabaseMeta).toHaveBeenCalledWith(projectInstanceId, "sales", 1),
    );
    const replacementStoreSnapshot = await replaceProject();
    metadata.resolve(oldMeta);
    await act(async () => completion);

    expect(rows).not.toHaveBeenCalled();
    expect(loader.loadedRows).toEqual([]);
    expect(loader.loadedRowIds).toEqual([]);
    expect(useResourceStore.getState()).toBe(replacementStoreSnapshot);
  });

  it("keeps a newer page when an older reload completes later", async () => {
    act(() =>
      useResourceStore.setState((state) => ({
        databases: {
          ...state.databases,
          sales: { ...state.databases.sales, rowCount: 400 },
        },
      })),
    );
    const oldPage = deferred<DatabaseRowsResult>();
    const nextPage = deferred<DatabaseRowsResult>();
    vi.spyOn(DatabaseService, "getDatabaseRows").mockImplementation(
      async (_projectInstanceId, _id, _expectedRevision, offset) =>
        offset === 0 ? oldPage.promise : nextPage.promise,
    );
    vi.spyOn(DatabaseService, "getDatabaseMeta").mockResolvedValue({
      ...oldMeta,
      rowCount: 400,
    });

    let reloadCompletion!: Promise<void>;
    let pageCompletion!: Promise<void>;
    act(() => {
      reloadCompletion = loader.reloadAllData();
    });
    await vi.waitFor(() =>
      expect(DatabaseService.getDatabaseRows).toHaveBeenCalledWith(
        projectInstanceId,
        "sales",
        1,
        0,
        loader.pageSize,
      ),
    );
    act(() => {
      pageCompletion = loader.goToNextPage();
    });
    nextPage.resolve({ rows: [[200]], rowIds: ["200"] });
    await act(async () => pageCompletion);
    oldPage.resolve({ rows: [[0]], rowIds: ["0"] });
    await act(async () => reloadCompletion);

    expect(loader.pageIndex).toBe(1);
    expect(loader.loadedRows).toEqual([[200]]);
    expect(loader.loadedRowIds).toEqual(["200"]);
  });

  it("keeps the current page and row references for semantic changes, and reloads for physical changes", async () => {
    const metadata = { ...oldMeta, rowCount: 600 };
    vi.spyOn(DatabaseService, "getDatabaseMeta").mockResolvedValue(metadata);
    const rows = vi
      .spyOn(DatabaseService, "getDatabaseRows")
      .mockImplementation(async (_project, _id, _revision, offset) => ({
        rows: [[offset]],
        rowIds: [String(offset)],
      }));
    await act(async () => loader.loadInitialRows("sales"));
    await act(async () => loader.goToNextPage());
    const previous = loader;
    const resource = { kind: "database" as const, id: "sales" };
    const key = resourceKey(resource);
    act(() => {
      const current = useResourceStore.getState();
      current.setSnapshot({
        resources: [{ ...current.resources[key], revision: 2 }],
        databases: {
          sales: {
            ...current.databases.sales,
            columns: [
              {
                ...metadata.columns[0],
                semantic: { kind: "Identifier", values: [], positiveValue: null, numeric: null },
              },
            ],
          },
        },
      });
    });
    await act(async () => loader.loadInitialRows("sales"));
    expect(loader.loadedRows).toBe(previous.loadedRows);
    expect(loader.loadedRowIds).toBe(previous.loadedRowIds);
    expect(loader.pageIndex).toBe(1);
    expect(loader.lastFetchMs).toBe(previous.lastFetchMs);
    expect(loader.loading).toBe(false);
    expect(rows).toHaveBeenCalledTimes(2);
    expect(DatabaseService.getDatabaseMeta).toHaveBeenCalledOnce();

    await act(async () => loader.goToNextPage());
    expect(rows).toHaveBeenLastCalledWith(projectInstanceId, "sales", 2, 400, loader.pageSize);
    act(() => {
      const current = useResourceStore.getState();
      current.setSnapshot({
        resources: [{ ...current.resources[key], revision: 3 }],
        databases: {
          sales: {
            ...current.databases.sales,
            dataRevision: "2",
            columns: [{ name: "amount", type: "Utf8" }],
          },
        },
      });
    });
    rows.mockResolvedValue({ rows: [["converted"]], rowIds: ["0"] });
    await act(async () => loader.loadInitialRows("sales"));
    expect(rows).toHaveBeenLastCalledWith(projectInstanceId, "sales", 3, 0, loader.pageSize);
    expect(rows).toHaveBeenCalledTimes(4);
    expect(loader.pageIndex).toBe(0);
    expect(loader.loadedRows).toEqual([["converted"]]);
    expect(loader.loadedRows).not.toBe(previous.loadedRows);

    vi.mocked(DatabaseService.getDatabaseMeta).mockResolvedValue({
      ...metadata,
      dataRevision: "2",
    });
    await act(async () => loader.reloadAllData());
    expect(rows).toHaveBeenCalledTimes(5);
  });

  it("suppresses delayed metadata rejection after replacement with zero effects", async () => {
    act(() =>
      useResourceStore.setState({
        databases: {
          sales: { id: "sales", name: "Sales", columns: [], rowCount: 0, columnCount: 0 },
        },
      }),
    );
    const metadata = deferred<DatabaseMetadataResult>();
    vi.spyOn(DatabaseService, "getDatabaseMeta").mockReturnValue(metadata.promise);
    const rows = vi.spyOn(DatabaseService, "getDatabaseRows");
    const warn = vi.spyOn(logger.data, "warn");

    const completion = loader.loadInitialRows("sales");
    await vi.waitFor(() => expect(DatabaseService.getDatabaseMeta).toHaveBeenCalled());
    const replacementStoreSnapshot = await replaceProject();
    metadata.reject(new Error("old metadata failed"));
    await act(async () => completion);

    expect(rows).not.toHaveBeenCalled();
    expect(warn).not.toHaveBeenCalled();
    expect(loader.loadedRows).toEqual([]);
    expect(loader.loadedRowIds).toEqual([]);
    expect(useResourceStore.getState()).toBe(replacementStoreSnapshot);
  });
});
