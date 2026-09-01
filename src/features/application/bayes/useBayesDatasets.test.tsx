// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import type { LoadDatabaseResult } from "@/shared/types/dto/database";
import { DatabaseService } from "@/services/database/databaseService";
import { useBayesDatasets, type BayesDatasetsModel } from "./useBayesDatasets";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock("@/services/database/databaseService", () => ({
  DatabaseService: {
    getDatabaseMeta: vi.fn(),
  },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}

const readyDatabase: DatabaseRecord = {
  id: "sales",
  name: "Sales",
  columns: [{ name: "amount", type: "Int64" }],
  rowCount: 1,
  columnCount: 1,
};

const metadata: LoadDatabaseResult = {
  id: "sales",
  name: "Sales loaded",
  columns: [{ name: "amount", type: "Int64" }],
  rowCount: 1,
  columnCount: 1,
};

describe("useBayesDatasets metadata ownership", () => {
  let host: HTMLDivElement;
  let root: Root;
  let model!: BayesDatasetsModel;

  function Harness() {
    model = useBayesDatasets();
    return null;
  }

  beforeEach(() => {
    vi.clearAllMocks();
    useDatabaseStore.setState({ databases: {}, revisions: {} });
    useProjectIOStore.setState({ projectInstanceId: null });
    startProjectLifecycle("project-1");
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    host.remove();
    useDatabaseStore.setState({ databases: {}, revisions: {} });
    useProjectIOStore.setState({ projectInstanceId: null });
    clearProjectLifecycle();
  });

  it("does not require a project identity when all database metadata is present", async () => {
    clearProjectLifecycle();
    useDatabaseStore.setState({ databases: { sales: readyDatabase }, revisions: {} });

    await act(async () => root.render(<Harness />));

    expect(DatabaseService.getDatabaseMeta).not.toHaveBeenCalled();
    expect(model.datasets[0]).toMatchObject({ sourceId: "sales", displayName: "Sales" });
  });

  it("does not publish metadata that resolves after project replacement", async () => {
    const request = deferred<LoadDatabaseResult>();
    vi.mocked(DatabaseService.getDatabaseMeta).mockImplementation((projectInstanceId) =>
      projectInstanceId === "project-1"
        ? request.promise
        : new Promise<LoadDatabaseResult>(() => undefined),
    );
    useDatabaseStore.setState({
      databases: { sales: { id: "sales", name: "Old sales" } },
      revisions: { sales: 1 },
    });
    useProjectIOStore.setState({ projectInstanceId: "project-1" });

    await act(async () => root.render(<Harness />));
    await vi.waitFor(() =>
      expect(DatabaseService.getDatabaseMeta).toHaveBeenCalledWith("project-1", "sales"),
    );

    await act(async () => {
      startProjectLifecycle("project-2");
      useProjectIOStore.setState({ projectInstanceId: "project-2" });
      request.resolve(metadata);
      await request.promise;
    });

    expect(useDatabaseStore.getState().databases.sales?.name).toBe("Old sales");
    expect(model.issue).toBeNull();
  });
});
