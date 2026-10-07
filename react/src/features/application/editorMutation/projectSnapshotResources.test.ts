import { expect, it } from "vitest";
import { resourceKey } from "@/features/core/resource";
import { projectSnapshotFixture } from "@/tests/helpers/projectSnapshotFixtures";
import { prepareSnapshotResources } from "./projectSnapshotResources";

it("remaps and removes resources atomically while retaining dirty and untouched document identities", () => {
  const { current, plan } = projectSnapshotFixture(3);
  const [moved, removed, unchanged] = plan.index.charts;
  const renamed = "charts/Renamed.yssbi-chart";
  const key = (path: string) => resourceKey({ id: path, kind: "chart" });
  current.documents[key(moved.chartPath)].dirty = true;
  const update = {
    ...plan,
    index: {
      ...plan.index,
      charts: [{ ...moved, chartPath: renamed, name: "Renamed" }, unchanged],
    },
    filePathRemaps: new Map([[moved.chartPath, renamed]]),
    deletedResources: new Set([key(removed.chartPath)]),
    chartDocuments: new Map([
      [renamed, { ...current.chartDocuments[moved.chartPath], databaseId: "external" }],
    ]),
  };
  const next = prepareSnapshotResources(update, current, [key(renamed)]);
  expect(next.chartDocuments[renamed]).toBe(current.chartDocuments[moved.chartPath]);
  expect(next.documents[key(renamed)]).toMatchObject({ resourceKey: key(renamed), dirty: true });
  expect(next.resources[key(unchanged.chartPath)]).toBe(
    current.resources[key(unchanged.chartPath)],
  );
  expect(next.documents[key(unchanged.chartPath)]).toBe(
    current.documents[key(unchanged.chartPath)],
  );
  expect(next.chartDocuments[unchanged.chartPath]).toBe(
    current.chartDocuments[unchanged.chartPath],
  );
  expect(next.resources[key(removed.chartPath)]).toBeUndefined();
  expect(next.chartDocuments[removed.chartPath]).toBeUndefined();
  expect(current.resources[key(moved.chartPath)].id).toBe(moved.chartPath);
  expect(current.documents[key(removed.chartPath)]).toBeDefined();
  expect(
    prepareSnapshotResources(
      { ...update, filePathRemaps: new Map(), deletedResources: new Set() },
      next,
      [],
    ),
  ).toBe(next);
});
