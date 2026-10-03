import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import {
  captureProjectIdentity,
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { DatabaseService, type DatabaseRowsResult } from "@/services/database/databaseService";
import type { LoadDatabaseResult } from "@/shared/types/domain/database";
import {
  captureDatabaseRead,
  hydrateDatabaseEditorMetadata,
  readDatabasePage,
} from "./databaseRead";

const database = { id: "sales", kind: "database" as const };
const key = resourceKey(database);
const metadata: LoadDatabaseResult = {
  id: database.id,
  name: "Sales",
  columns: [{ name: "value", type: "Float64" }],
  rowCount: 2,
  columnCount: 1,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

beforeEach(() => {
  startProjectLifecycle("database-read-project");
  useResourceStore.getState().clear();
  useResourceStore.getState().setSnapshot({
    databases: { [database.id]: { ...metadata, rowCount: 1 } },
    resources: [
      {
        ...database,
        uri: key,
        name: "Sales",
        revision: 1,
        exists: true,
        loaded: true,
        hasDirtyDocument: false,
        hasStaleDocument: false,
        hasConflictDocument: false,
      },
    ],
    publicationRevision: 1,
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  useResourceStore.getState().clear();
  clearProjectLifecycle();
});

it("does not install delayed metadata after the same database advances revision", async () => {
  const pending = deferred<LoadDatabaseResult>();
  const query = vi.spyOn(DatabaseService, "getDatabaseMeta").mockReturnValue(pending.promise);
  const completion = hydrateDatabaseEditorMetadata(database.id);
  expect(query).toHaveBeenCalledWith(captureProjectIdentity().projectInstanceId, database.id, 1);
  useResourceStore.getState().patchResource(database, { revision: 2 });
  const replacement = useResourceStore.getState();
  pending.resolve(metadata);
  await completion;
  expect(useResourceStore.getState()).toBe(replacement);
  expect(replacement.databases.sales.rowCount).toBe(1);
});

it("rejects a delayed page and prevents an obsolete read from starting another request", async () => {
  const pending = deferred<DatabaseRowsResult>();
  const query = vi.spyOn(DatabaseService, "getDatabaseRows").mockReturnValueOnce(pending.promise);
  const identity = captureProjectIdentity();
  const read = captureDatabaseRead(identity, database.id);
  const completion = readDatabasePage(read, 0, 200);
  useResourceStore.getState().patchResource(database, { revision: 2 });
  pending.resolve({ rows: [[1]], rowIds: ["10"] });
  expect(await completion).toBeNull();
  expect(await readDatabasePage(read, 1, 200)).toBeNull();
  expect(query).toHaveBeenCalledOnce();

  const page = { rows: [[2]], rowIds: ["20"] };
  query.mockResolvedValue(page);
  let active = true;
  const current = captureDatabaseRead(identity, database.id, () => active);
  const accepted = await readDatabasePage(current, 9, 200);
  expect(accepted).toMatchObject({ ...page, pageIndex: 0 });
  expect(accepted?.rows).toBe(page.rows);
  expect(accepted?.rowIds).toBe(page.rowIds);
  expect(query).toHaveBeenLastCalledWith(identity.projectInstanceId, database.id, 2, 0, 200);
  active = false;
  expect(await readDatabasePage(current, 0, 200)).toBeNull();
  expect(query).toHaveBeenCalledTimes(2);

  active = true;
  query.mockResolvedValue({ rows: [[1, 2]], rowIds: ["20"] });
  await expect(readDatabasePage(current, 0, 200)).rejects.toThrow(
    "Database page width does not match its metadata",
  );
});
