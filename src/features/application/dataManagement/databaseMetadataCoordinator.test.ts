import { describe, expect, it } from "vitest";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import type {
  DatabaseMetadataCoordinatorDependencies,
  DatabaseMetadataOutcome,
} from "./databaseMetadataCoordinator";
import { DatabaseMetadataCoordinator } from "./databaseMetadataCoordinator";

interface Deferred<T> {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
  readonly reject: (reason: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((settle, fail) => {
    resolve = settle;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function database(id: string, name: string): DatabaseRecord {
  return {
    id,
    name,
    columns: [{ name: "value", type: "Int64" }],
    rowCount: 1,
    columnCount: 1,
  };
}

function setup(): {
  dependencies: DatabaseMetadataCoordinatorDependencies;
  coordinator: DatabaseMetadataCoordinator;
  reader: { read: DatabaseMetadataCoordinatorDependencies["reader"]["read"] };
  currentProject: { projectInstanceId: string; epoch: number };
  published: Array<{ readonly name: string }>;
  failures: string[];
} {
  const currentProject = { projectInstanceId: "project-a", epoch: 1 };
  const published: Array<{ readonly name: string }> = [];
  const failures: string[] = [];
  const reader = {
    read: async () => database("unused", "unused"),
  };
  const dependencies: DatabaseMetadataCoordinatorDependencies = {
    project: {
      capture: () => ({ ...currentProject }),
      isCurrent: (identity) =>
        identity.projectInstanceId === currentProject.projectInstanceId &&
        identity.epoch === currentProject.epoch,
    },
    reader,
    publication: {
      publishDatabase: (record) => published.push({ name: record.name }),
      publishDatabaseFailure: (id) => failures.push(id),
    },
  };
  return {
    dependencies,
    coordinator: new DatabaseMetadataCoordinator(dependencies),
    reader,
    currentProject,
    published,
    failures,
  };
}

describe("DatabaseMetadataCoordinator", () => {
  it("publishes neither stale success nor stale failure after project replacement", async () => {
    const fixture = setup();
    const request = deferred<DatabaseRecord>();
    fixture.reader.read = () => request.promise;

    const completion = fixture.coordinator.load("sales");
    fixture.currentProject.projectInstanceId = "project-b";
    fixture.currentProject.epoch = 2;
    request.resolve(database("sales", "old project"));

    await expect(completion).resolves.toEqual({
      status: "stale",
    } satisfies DatabaseMetadataOutcome);
    expect(fixture.published).toEqual([]);
    expect(fixture.failures).toEqual([]);
  });

  it("only publishes the newest request for one database while another key remains independent", async () => {
    const fixture = setup();
    const oldSales = deferred<DatabaseRecord>();
    const newSales = deferred<DatabaseRecord>();
    const inventory = deferred<DatabaseRecord>();
    const requests = new Map<string, Deferred<DatabaseRecord>[]>([
      ["sales", [oldSales, newSales]],
      ["inventory", [inventory]],
    ]);
    fixture.reader.read = async (_projectId, id) =>
      requests.get(id)?.shift()?.promise ?? database(id, "unexpected");

    const oldCompletion = fixture.coordinator.load("sales");
    const newCompletion = fixture.coordinator.load("sales");
    const inventoryCompletion = fixture.coordinator.load("inventory");

    newSales.resolve(database("sales", "newest"));
    inventory.resolve(database("inventory", "independent"));
    oldSales.resolve(database("sales", "stale"));

    await expect(newCompletion).resolves.toEqual({
      status: "published",
    } satisfies DatabaseMetadataOutcome);
    await expect(inventoryCompletion).resolves.toEqual({
      status: "published",
    } satisfies DatabaseMetadataOutcome);
    await expect(oldCompletion).resolves.toEqual({
      status: "stale",
    } satisfies DatabaseMetadataOutcome);
    expect(fixture.published.map((record) => record.name)).toEqual(["newest", "independent"]);
  });
});
