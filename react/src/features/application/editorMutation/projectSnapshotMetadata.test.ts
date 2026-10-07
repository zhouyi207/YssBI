import { useResourceStore } from "@/features/core/resource/resourceStore";
import { beforeEach, expect, it } from "vitest";
import { projectSnapshotFixture } from "@/tests/helpers/projectSnapshotFixtures";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import type { ProjectFunctionGraphIndexRow } from "@/shared/types/domain/project";
import { prepareProjectSnapshotCommit } from "./projectPublicationSnapshot";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import { prepareGraphMetaSnapshot } from "@/features/core/dataStore/graphMeta";
import {
  buildAuthoritativeProjectLoadPlan,
  defaultAuthoritativeProjectLoadPlanDependencies,
} from "@/features/application/project/authoritativeProjectLoadPlan";

beforeEach(() => {
  useResourceStore.getState().clear();
});

it("shares current database metadata and discards it when the indexed revision changes", () => {
  const { plan } = projectSnapshotFixture(0);
  plan.index.databases = ["stable", "changed", "removed"].map((id) => ({
    id,
    resourcePath: `databases/${id}.yssbi-database`,
    name: id,
    revision: 1,
    engine: { dataset: {} },
    schemaVersion: 1,
    required: false,
  }));
  const initial = prepareProjectSnapshotCommit(plan).storeState;
  const columns = [
    {
      name: "value",
      type: "Float64",
      physical: "Float64",
      semantic: null,
      supportedSemanticTypes: ["Numeric" as const],
    },
  ];
  const databases: Record<string, DatabaseRecord> = {
    ...initial.databases,
    changed: { ...initial.databases.changed, columns, rowCount: 5, columnCount: 1 },
  };
  const loaded = buildAuthoritativeProjectLoadPlan(
    {
      path: null,
      index: plan.index,
      databases: Object.fromEntries(
        plan.index.databases.map(({ id, engine, schemaVersion, required }) => [
          id,
          {
            id,
            engine,
            schemaVersion,
            required,
            name: "Runtime name",
            loadFailed: false,
            columns: id === "changed" ? columns : [],
            rowCount: id === "changed" ? 5 : 0,
            columnCount: id === "changed" ? 1 : 0,
          },
        ]),
      ),
    },
    { databases: {}, resources: {}, detailFocus: null },
    { ...defaultAuthoritativeProjectLoadPlanDependencies, validateCoordinatorStart: () => {} },
  ).storeState;
  expect(loaded.databases.changed).toMatchObject({ name: "changed", rowCount: 5, columnCount: 1 });
  expect(loaded.resources[resourceKey({ kind: "database", id: "changed" })].revision).toBe(1);
  useResourceStore
    .getState()
    .setSnapshot({ databases, resources: Object.values(initial.resources) });

  const repeated = prepareProjectSnapshotCommit({
    ...plan,
    index: structuredClone(plan.index),
  }).storeState;
  expect(repeated.databases).toBe(databases);
  const key = (id: string) => resourceKey({ kind: "database", id });
  expect(repeated.resources[key("stable")]).toBe(initial.resources[key("stable")]);

  const index = structuredClone(plan.index);
  index.databases.pop();
  index.databases[1].name = "Renamed";
  index.databases[1].revision = 2;
  const next = prepareProjectSnapshotCommit({ ...plan, index }).storeState;
  expect(next.databases.stable).toBe(databases.stable);
  expect(next.databases.changed.name).toBe("Renamed");
  expect(next.databases.changed.rowCount).toBeUndefined();
  expect(next.databases.changed.columnCount).toBeUndefined();
  expect(next.databases.changed.columns).toBeUndefined();
  expect(next.databases.changed.engine).toBe(databases.changed.engine);
  expect(next.databases.removed).toBeUndefined();
  expect(next.resources[key("stable")].revision).toBe(1);
  expect(next.resources[key("changed")].revision).toBe(2);
  expect(next.resources[key("removed")]).toBeUndefined();
  expect(databases.changed.name).toBe("changed");
  expect(initial.resources[key("changed")].revision).toBe(1);
  expect(useResourceStore.getState().databases).toBe(databases);
});

function functionRow(name: string): ProjectFunctionGraphIndexRow {
  return {
    path: `functions/${name}.yssbi-function`,
    name,
    type: "function_graph",
    revision: 1,
    functionRevision: 1,
    functionSignature: {
      parameters: [{ id: "input", name: "Input", type_name: "Numeric" }],
      return_type: "Text",
    },
    functionEditorProjection: {
      functionRevision: 1,
      inputs: [{ id: "input", name: "Input", dataType: { kind: "Scalar", inner: "Numeric" } }],
      outputs: [{ id: "return", name: "Output", dataType: { kind: "Scalar", inner: "Text" } }],
    },
  };
}

it("shares graph metadata branches and isolates changed external signature data", () => {
  const { plan } = projectSnapshotFixture(0);
  const stable = functionRow("Stable");
  const changed = functionRow("Changed");
  const removed = "events/Removed.yssbi-event";
  plan.index.functionGraphs = [stable, changed];
  plan.index.eventGraphs = [{ path: removed, name: "Removed", type: "event_graph", revision: 1 }];
  const initial = prepareProjectSnapshotCommit(plan).storeState.graphMeta;
  useResourceStore.setState({ graphMeta: initial });

  const repeated = prepareProjectSnapshotCommit({
    ...plan,
    index: structuredClone(plan.index),
  }).storeState.graphMeta;
  expect(repeated).toBe(initial);

  const index = structuredClone(plan.index);
  index.eventGraphs = [];
  const update = index.functionGraphs[1];
  update.revision = update.functionRevision = update.functionEditorProjection.functionRevision = 2;
  update.functionSignature.parameters[0].name = "Renamed";
  update.functionEditorProjection.inputs[0].name = "Renamed";
  const next = prepareProjectSnapshotCommit({ ...plan, index }).storeState.graphMeta;
  expect(next[stable.path]).toBe(initial[stable.path]);
  expect(next[changed.path].functionRevision).toBe(2);
  expect(next[changed.path].functionOutputs).toBe(initial[changed.path].functionOutputs);
  expect(next[changed.path].functionInputs![0].dataType).toBe(
    initial[changed.path].functionInputs![0].dataType,
  );
  expect(next[removed]).toBeUndefined();
  update.functionSignature.parameters[0].name = "External mutation";
  update.functionEditorProjection.inputs[0].name = "External mutation";
  expect(next[changed.path].functionSignature!.parameters[0].name).toBe("Renamed");
  expect(next[changed.path].functionInputs![0].name).toBe("Renamed");
  expect(initial[changed.path].functionInputs![0].name).toBe("Input");
  expect(useResourceStore.getState().graphMeta).toBe(initial);
});

it("updates recursive function types and removals even when the function revision is unchanged", () => {
  const row = functionRow("Nested");
  row.functionEditorProjection.inputs[0].dataType = {
    kind: "OneOf",
    inner: [
      { kind: "Array", inner: { kind: "Scalar", inner: "Numeric" } },
      { kind: "Struct", inner: "Retained" },
    ],
  };
  const before = prepareGraphMetaSnapshot([row]);
  const incoming = structuredClone(row);
  incoming.functionSignature.parameters = [];
  incoming.functionSignature.return_type = null;
  incoming.functionEditorProjection.outputs = [];
  const changedType = {
    kind: "OneOf" as const,
    inner: [
      { kind: "DataSeries" as const, inner: { kind: "Scalar" as const, inner: "Text" as const } },
      { kind: "Struct" as const, inner: "Retained" },
    ],
  };
  incoming.functionEditorProjection.inputs[0].dataType = changedType;
  const after = prepareGraphMetaSnapshot([incoming], before);
  const previousType = before[row.path].functionInputs![0].dataType;
  const nextType = after[row.path].functionInputs![0].dataType;
  expect(after[row.path].functionRevision).toBe(before[row.path].functionRevision);
  expect(after[row.path].functionSignature).toEqual({ parameters: [], return_type: null });
  expect(after[row.path].functionOutputs).toEqual([]);
  expect(nextType).toEqual(changedType);
  if (previousType.kind !== "OneOf" || nextType.kind !== "OneOf") throw new Error("Expected OneOf");
  expect(nextType.inner[1]).toBe(previousType.inner[1]);
  changedType.inner[1].inner = "External mutation";
  expect(nextType.inner[1]).toEqual({ kind: "Struct", inner: "Retained" });
  expect(previousType.inner[0]).toEqual({
    kind: "Array",
    inner: { kind: "Scalar", inner: "Numeric" },
  });

  const event = prepareGraphMetaSnapshot(
    [{ path: row.path, name: row.name, type: "event_graph", revision: row.revision }],
    after,
  );
  expect(event[row.path]).toEqual({ type: "event_graph" });
  expect(after[row.path].functionSignature).toEqual({ parameters: [], return_type: null });
});
