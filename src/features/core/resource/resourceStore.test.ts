import { beforeEach, expect, it } from "vitest";
import {
  buildFileResourceMeta,
  clearResourceDocumentState,
  markResourceDirty,
  resourceKey,
  useResourceStore,
} from "./index";
import { getResourceSnapshot } from "./read";

beforeEach(() => {
  useResourceStore.getState().clear();
});

it("limits database metadata writes and clears their declarations with the resource snapshot", () => {
  const database = {
    id: "sales",
    name: "Sales",
    columns: [
      {
        name: "value",
        type: "Float64",
        physical: "Float64",
        semantic: null,
        supportedSemanticTypes: [
          "Numeric",
          "Categorical",
          "Ordinal",
          "Binary",
          "Identifier",
        ] as const,
      },
    ],
    rowCount: 1,
    columnCount: 1,
  };
  const key = resourceKey({ kind: "database", id: database.id });
  useResourceStore.getState().setSnapshot({
    databases: { sales: database, other: { id: "other", name: "Other" } },
    resources: [
      {
        id: database.id,
        name: database.name,
        kind: "database",
        uri: key,
        revision: 4,
        exists: true,
        loaded: true,
        hasDirtyDocument: false,
        hasStaleDocument: false,
        hasConflictDocument: false,
      },
    ],
    publicationRevision: 4,
  });
  const before = useResourceStore.getState();
  const observed: unknown[] = [];
  const stop = useResourceStore.subscribe((state) =>
    observed.push({
      databases: state.databases,
      resources: state.resources,
      publication: state.indexRevision,
    }),
  );
  try {
    const metadata = { ...structuredClone(database), id: "unexpected", name: "Stale name" };
    useResourceStore.getState().updateDatabaseMetadata("sales", 4, metadata);
    expect(useResourceStore.getState()).toBe(before);
    expect(observed).toEqual([]);

    useResourceStore.getState().updateDatabaseMetadata("sales", 3, { ...metadata, rowCount: 9 });
    expect(useResourceStore.getState()).toBe(before);
    useResourceStore.getState().updateDatabaseMetadata("sales", 4, { ...metadata, rowCount: 2 });
    const after = useResourceStore.getState();
    expect(observed).toHaveLength(1);
    expect(after.databases.sales).toMatchObject({ id: "sales", name: "Sales", rowCount: 2 });
    expect(after.databases.sales.columns).toBe(before.databases.sales.columns);
    expect(after.databases.other).toBe(before.databases.other);
    expect(after.resources).toBe(before.resources);
    expect(before.databases.sales.rowCount).toBe(1);

    after.clear();
    expect(observed).toHaveLength(2);
    expect(observed[1]).toEqual({ databases: {}, resources: {}, publication: 0 });
  } finally {
    stop();
  }
});

it("publishes document flags and resource summaries together and ignores repeated flags", () => {
  const resource = buildFileResourceMeta("doc", "docs/Report.yssbi-doc", "Report");
  const key = resourceKey(resource);
  useResourceStore.getState().setSnapshot({ resources: [resource] });
  const observed: unknown[] = [];
  const stop = useResourceStore.subscribe(() => {
    const snapshot = getResourceSnapshot();
    observed.push({
      dirty: snapshot.documents[key]?.dirty,
      loaded: snapshot.documents[key]?.loaded,
      stale: snapshot.documents[key]?.stale,
      summaryDirty: snapshot.resources[key].hasDirtyDocument,
      summaryLoaded: snapshot.resources[key].loaded,
      summaryStale: snapshot.resources[key].hasStaleDocument,
    });
  });
  try {
    markResourceDirty(resource, true);
    expect(observed).toEqual([
      {
        dirty: true,
        loaded: true,
        stale: false,
        summaryDirty: true,
        summaryLoaded: true,
        summaryStale: false,
      },
    ]);
    const changed = getResourceSnapshot();
    markResourceDirty(resource, true);
    expect(observed).toHaveLength(1);
    expect(getResourceSnapshot()).toBe(changed);
    clearResourceDocumentState(resource);
    expect(observed).toHaveLength(2);
    expect(observed[1]).toEqual({
      dirty: undefined,
      loaded: undefined,
      stale: undefined,
      summaryDirty: false,
      summaryLoaded: false,
      summaryStale: false,
    });
    expect(changed.documents[key].dirty).toBe(true);
    expect(changed.resources[key].hasDirtyDocument).toBe(true);

    useResourceStore.getState().setSnapshot({
      resources: [resource],
      documents: changed.documents,
    });
    expect(observed).toHaveLength(3);
    expect(observed[2]).toEqual(observed[0]);
    useResourceStore.getState().markAllStale();
    expect(observed).toHaveLength(4);
    expect(observed[3]).toEqual({
      dirty: true,
      loaded: true,
      stale: true,
      summaryDirty: true,
      summaryLoaded: true,
      summaryStale: true,
    });
    const stale = getResourceSnapshot();
    useResourceStore.getState().markAllStale();
    expect(observed).toHaveLength(4);
    expect(getResourceSnapshot()).toBe(stale);
    expect(changed.documents[key].stale).toBe(false);
  } finally {
    stop();
  }
});

it("retains snapshot references for unchanged content and publishes membership and order once", () => {
  const stable = buildFileResourceMeta("event_graph", "events/Stable.yssbi-event", "Stable");
  const changed = buildFileResourceMeta("event_graph", "events/Changed.yssbi-event", "Changed");
  const removed = buildFileResourceMeta("doc", "docs/Removed.yssbi-doc", "Removed");
  const resources = [stable, changed, removed];
  useResourceStore.getState().setSnapshot({ resources, publicationRevision: 1 });
  const before = getResourceSnapshot();
  const observed: unknown[] = [];
  const stop = useResourceStore.subscribe((state) =>
    observed.push({ revision: state.indexRevision, snapshot: getResourceSnapshot() }),
  );
  try {
    useResourceStore.getState().setSnapshot({
      resources: structuredClone(resources),
      graphOrder: [stable.id, changed.id],
      publicationRevision: 1,
    });
    expect(observed).toEqual([]);
    expect(getResourceSnapshot()).toBe(before);

    useResourceStore.getState().setSnapshot({ resources, publicationRevision: 2 });
    expect(observed).toEqual([{ revision: 2, snapshot: before }]);
    observed.length = 0;
    useResourceStore.getState().setSnapshot({
      resources: [{ ...changed, name: "Renamed" }, { ...stable }],
      publicationRevision: 3,
    });
    expect(observed).toHaveLength(1);
    const next = getResourceSnapshot();
    expect(next.resources[resourceKey(stable)]).toBe(before.resources[resourceKey(stable)]);
    expect(next.resources[resourceKey(changed)].name).toBe("Renamed");
    expect(next.resources[resourceKey(removed)]).toBeUndefined();
    expect(next.graphOrder).toEqual([changed.id, stable.id]);
    expect(before.resources[resourceKey(changed)].name).toBe("Changed");
    expect(before.graphOrder).toEqual([stable.id, changed.id]);
  } finally {
    stop();
  }
});
