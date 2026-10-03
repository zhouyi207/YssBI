import {
  installGraphProjectionFixture,
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  ProjectPublicationCoordinator,
  type ProjectPublicationDependencies,
} from "./projectPublicationCoordinator";
import {
  commitPreparedProjectSnapshot,
  prepareProjectSnapshotCommit,
} from "./projectPublicationSnapshot";
import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import type { ProjectIndexRow } from "@/shared/types/domain/project";
import type { DatabaseMetadataResult } from "@/shared/types/domain/database";
import {
  useResourceStore,
  buildFileResourceMeta,
  markResourceLoaded,
  markResourceDirty,
  resourceKey,
} from "@/features/core/resource";

import { getResourceSnapshot } from "@/features/core/resource/read";

import { captureProjectLifecycleState } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useNodeCatalogStore } from "@/features/core/nodeCatalog/nodeCatalogStore";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { useViewportStore, viewportScopeKey } from "@/features/core/viewport";

const eventPath = "events/Event.yssbi-event";

function index(revision: number): ProjectIndexRow {
  return {
    projectInstanceId: "project-a",
    projectName: "Project",
    exportTime: "",
    publicationRevision: revision,
    eventGraphs: [],
    functionGraphs: [],
    minds: [],
    docs: [],
    charts: [],
    databases: [],
  };
}
function receipt(
  revision: number,
  path = eventPath,
  kind: "event_graph" | "chart" = "event_graph",
): ResourceMutationResultDto {
  const operationId = `00000000-0000-0000-0000-${String(revision).padStart(12, "0")}`;
  return {
    projectInstanceId: "project-a",
    operationId,
    publicationRevision: revision,
    moves: [],
    deltas: [
      {
        resource: { kind: kind === "event_graph" ? "graph" : "chart", key: path },
        fromRevision: 0,
        toRevision: 0,
        causedBy: operationId,
        payload: {
          kind: "resource_lifecycle",
          patch: { before: null, after: { path, kind, name: "Created", revision: 0 } },
        },
      },
    ],
    projectionReplacements: [],
    projectionStatus: { status: "complete", expectedGraphPaths: [] },
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
let coordinator: ProjectPublicationCoordinator;
function setup(overrides: Partial<ProjectPublicationDependencies> = {}) {
  const dependencies = {
    loadProjectIndex: vi.fn(async () => projectIndexSnapshotFixture(index(0))),
    loadChartDocument: vi.fn(),
    loadDatabaseMetadata: vi.fn(),
    prepareGraphSession: vi.fn(async (path: string) =>
      makeGraphEditorSession(makeEditorProjectionFixture({ graphPath: path }).projection),
    ),
    captureLoadedGraphPaths: () => new Set(Object.keys(useResourceStore.getState().graphEntities)),
    prepareSnapshot: prepareProjectSnapshotCommit,
    commitSnapshot: vi.fn(commitPreparedProjectSnapshot),
    markProjectProjectionStale: vi.fn(),
    ...overrides,
  };
  coordinator = new ProjectPublicationCoordinator(dependencies);
  coordinator.startProject("project-a", 0);
  return dependencies;
}
beforeEach(() => {
  useResourceStore.getState().clear();
});
afterEach(() => coordinator?.cancelProject());

it("publishes database declarations, resource revisions and the project watermark together", async () => {
  let next = index(1);
  next.databases = [
    {
      id: "sales",
      name: "Sales",
      resourcePath: "databases/sales.yssbi-database",
      revision: 1,
      engine: { dataset: {} },
      schemaVersion: 1,
      required: false,
    },
  ];
  setup({ loadProjectIndex: vi.fn(async () => projectIndexSnapshotFixture(next)) });
  const observed: unknown[] = [];
  const stop = useResourceStore.subscribe((state) => {
    const resources = useResourceStore.getState();
    const resource = resources.resources[resourceKey({ kind: "database", id: "sales" })];
    observed.push({
      database: state.databases.sales?.name,
      resource: resource?.name,
      revision: resource?.revision,
      publication: resources.indexRevision,
    });
  });
  try {
    await coordinator.refreshIndex();
    next = {
      ...next,
      publicationRevision: 2,
      databases: [{ ...next.databases[0], name: "Renamed", revision: 2 }],
    };
    await coordinator.refreshIndex();
    next = { ...next, publicationRevision: 3, databases: [] };
    await coordinator.refreshIndex();
  } finally {
    stop();
  }
  expect(observed).toEqual([
    { database: "Sales", resource: "Sales", revision: 1, publication: 1 },
    { database: "Renamed", resource: "Renamed", revision: 2, publication: 2 },
    { database: undefined, resource: undefined, revision: undefined, publication: 3 },
  ]);
});

it("publishes refreshed semantic metadata atomically without discarding the row-data revision", async () => {
  let next = index(1);
  next.databases = ["sales", "stable"].map((id) => ({
    id,
    name: id,
    resourcePath: `databases/${id}`,
    revision: 1,
    engine: { dataset: {} },
    schemaVersion: 1,
    required: false,
  }));
  const pending = deferred<DatabaseMetadataResult>();
  const dependencies = setup({
    loadProjectIndex: vi.fn(async () => projectIndexSnapshotFixture(next)),
    loadDatabaseMetadata: vi.fn(() => pending.promise),
  });
  await coordinator.refreshIndex();
  const metadata: DatabaseMetadataResult = {
    id: "sales",
    name: "Runtime name",
    rowCount: 400,
    columnCount: 1,
    dataRevision: "7",
    columns: [{ name: "code", type: "Int64", physical: "Int64", semantic: null }],
  };
  for (const id of ["sales", "stable"])
    useResourceStore.getState().updateDatabaseMetadata(id, 1, metadata);
  const before = useResourceStore.getState();
  next = {
    ...next,
    publicationRevision: 2,
    databases: next.databases.map((row) => (row.id === "sales" ? { ...row, revision: 2 } : row)),
  };
  const observed: unknown[] = [];
  const stop = useResourceStore.subscribe((state) =>
    observed.push({
      database: state.databases.sales,
      revision: state.resources[resourceKey({ kind: "database", id: "sales" })].revision,
    }),
  );
  try {
    const completion = coordinator.refreshIndex();
    await vi.waitFor(() =>
      expect(dependencies.loadDatabaseMetadata).toHaveBeenCalledWith("project-a", "sales", 2),
    );
    expect(useResourceStore.getState()).toBe(before);
    pending.resolve({
      ...metadata,
      columns: [
        {
          ...metadata.columns[0],
          semantic: { kind: "Identifier", values: [], positiveValue: null, numeric: null },
        },
      ],
    });
    await completion;
    expect(dependencies.loadDatabaseMetadata).toHaveBeenCalledOnce();
    expect(observed).toEqual([
      {
        revision: 2,
        database: expect.objectContaining({
          name: "sales",
          rowCount: 400,
          columnCount: 1,
          dataRevision: "7",
        }),
      },
    ]);
    const after = useResourceStore.getState();
    expect(after.databases.sales.columns?.[0].semantic?.kind).toBe("Identifier");
    expect(after.databases.stable).toBe(before.databases.stable);
    expect(before.databases.sales.columns?.[0].semantic).toBeNull();
  } finally {
    stop();
  }
});

it("applies late delete authorization to retained dirty content after an index-only refresh", async () => {
  const dependencies = setup({
    loadProjectIndex: vi.fn(async () => projectIndexSnapshotFixture(index(1))),
  });
  const path = "docs/Report.md";
  const ref = { id: path, kind: "doc" as const };
  useResourceStore
    .getState()
    .setSnapshot({ resources: [buildFileResourceMeta("doc", path, "Report", { revision: 0 })] });
  markResourceLoaded(ref);
  markResourceDirty(ref, true);
  useResourceStore.getState().installFileSnapshot({
    projectInstanceId: "project-a",
    path,
    kind: "doc",
    content: "unsaved",
    dirty: true,
    version: { sessionId: "doc-a", revision: 0 },
  });
  await coordinator.refreshIndex();
  expect(useResourceStore.getState().resources[resourceKey(ref)]).toMatchObject({
    exists: false,
    hasDirtyDocument: true,
  });
  const deletion = receipt(1);
  deletion.deltas = [
    {
      resource: { kind: "doc", key: path },
      fromRevision: 0,
      toRevision: 1,
      causedBy: deletion.operationId,
      payload: {
        kind: "resource_lifecycle",
        patch: { before: { path, kind: "doc", name: "Report", revision: 0 }, after: null },
      },
    },
  ];
  await coordinator.submit({ result: deletion });
  expect(useResourceStore.getState().resources[resourceKey(ref)]).toBeUndefined();
  expect(useResourceStore.getState().fileSnapshots.doc[path]).toBeUndefined();
  expect((await coordinator.submit({ result: deletion })).status).toBe("duplicate");
  expect(dependencies.loadProjectIndex).toHaveBeenCalledTimes(2);
});

it("coalesces command, event and index refreshes, including a snapshot ahead of a late receipt", async () => {
  const pendingIndex = deferred<ProjectIndexRow>();
  const dependencies = setup({
    loadProjectIndex: vi.fn(() => pendingIndex.promise.then(projectIndexSnapshotFixture)),
  });
  const identity = captureProjectLifecycleState();
  const first = receipt(1);
  const second = receipt(2, "events/Later.yssbi-event");
  const command = coordinator.submit({ result: first });
  const event = coordinator.submit({ result: structuredClone(first) });
  const watcher = coordinator.refreshIndex();
  await Promise.resolve();
  expect(dependencies.commitSnapshot).not.toHaveBeenCalled();
  const snapshot = index(2);
  snapshot.eventGraphs = [eventPath, "events/Later.yssbi-event"].map((path) => ({
    path,
    name: path,
    type: "event_graph",
    revision: 0,
  }));
  pendingIndex.resolve(snapshot);
  expect((await Promise.all([command, event])).map((outcome) => outcome.status)).toEqual([
    "applied",
    "applied",
  ]);
  await watcher;
  expect((await coordinator.submit({ result: second })).status).toBe("duplicate");
  expect(dependencies.loadProjectIndex).toHaveBeenCalledOnce();
  expect(dependencies.commitSnapshot).toHaveBeenCalledOnce();
  expect(useResourceStore.getState().graphOrder).toEqual(
    snapshot.eventGraphs.map((graph) => graph.path),
  );
  expect(coordinator.capturePublicationRevision()).toBe(2);
  expect(captureProjectLifecycleState()).toEqual(identity);
  expect(dependencies.markProjectProjectionStale).not.toHaveBeenCalled();
});

it("does not republish unchanged index content or invalidate catalogs on repeated watcher notifications", async () => {
  const snapshot = index(0);
  const dependencies = setup({
    loadProjectIndex: vi.fn(async () =>
      projectIndexSnapshotFixture({ ...snapshot, exportTime: String(Date.now()) }),
    ),
  });
  await coordinator.refreshIndex();
  const before = useResourceStore.getState();
  await coordinator.refreshIndex();
  expect(dependencies.commitSnapshot).toHaveBeenCalledOnce();
  expect(useResourceStore.getState()).toBe(before);
});

it("publishes revision-only receipts and indexes without replacing unchanged resource content", async () => {
  let snapshot = index(0);
  const dependencies = setup({
    loadProjectIndex: vi.fn(async () => projectIndexSnapshotFixture(snapshot)),
  });
  await coordinator.refreshIndex();
  const before = getResourceSnapshot();
  const revisions: number[] = [];
  const stop = useResourceStore.subscribe((state) => revisions.push(state.indexRevision));
  try {
    const empty = { ...receipt(1), deltas: [] };
    expect((await coordinator.submit({ result: empty })).status).toBe("applied");
    expect.soft(useResourceStore.getState().indexRevision).toBe(1);
    expect(dependencies.loadProjectIndex).toHaveBeenCalledOnce();

    snapshot = { ...snapshot, publicationRevision: 2 };
    await coordinator.refreshIndex();
    expect.soft(useResourceStore.getState().indexRevision).toBe(2);
    await coordinator.refreshIndex();
    expect.soft(revisions).toEqual([1, 2]);
    expect(coordinator.capturePublicationRevision()).toBe(2);
    expect(dependencies.commitSnapshot).toHaveBeenCalledOnce();
    expect(getResourceSnapshot()).toBe(before);
  } finally {
    stop();
  }
});

it("recovers loaded clean graphs for incomplete receipts even when index content is unchanged", async () => {
  const dirtyPath = "events/Dirty.yssbi-event";
  const snapshot = index(0);
  snapshot.eventGraphs = [eventPath, dirtyPath].map((path) => ({
    path,
    name: path,
    type: "event_graph",
    revision: 0,
  }));
  const dependencies = setup({
    loadProjectIndex: vi.fn(async () =>
      projectIndexSnapshotFixture({ ...snapshot, publicationRevision: 2 }),
    ),
    prepareGraphSession: vi.fn(async (path) =>
      makeGraphEditorSession(
        makeEditorProjectionFixture({ graphPath: path, title: "Recovered" }).projection,
      ),
    ),
  });
  coordinator.startProject("project-a", 0, snapshot);
  useResourceStore.getState().setSnapshot({
    resources: snapshot.eventGraphs.map((graph) =>
      buildFileResourceMeta("event_graph", graph.path, graph.name, { revision: 0 }),
    ),
  });
  for (const path of [eventPath, dirtyPath]) {
    const session = makeGraphEditorSession(
      makeEditorProjectionFixture({ graphPath: path }).projection,
    );
    session.editing.dirty = path === dirtyPath;
    useResourceStore.getState().installGraphSession(path, session, { mode: "load" });
    markResourceLoaded({ kind: "event_graph", id: path });
  }
  markResourceDirty({ kind: "event_graph", id: dirtyPath }, true);
  const previous = useResourceStore.getState();
  const incomplete: ResourceMutationResultDto = {
    ...receipt(2),
    deltas: [],
    projectionStatus: { status: "incomplete", invalidatedGraphPaths: [eventPath] },
  };
  expect((await coordinator.submit({ result: incomplete })).status).toBe("recovered");
  expect(dependencies.prepareGraphSession).toHaveBeenCalledExactlyOnceWith(
    eventPath,
    "project-a",
    captureProjectLifecycleState().epoch,
  );
  const current = useResourceStore.getState();
  expect(current.sessions[eventPath].projection.nodes[0].display.title).toBe("Recovered");
  expect(current.sessions[dirtyPath]).toBe(previous.sessions[dirtyPath]);
  expect(previous.sessions[eventPath].projection.nodes[0].display.title).toBe("Projected node");
  await coordinator.refreshIndex();
  expect(dependencies.prepareGraphSession).toHaveBeenCalledOnce();
  expect(dependencies.commitSnapshot).toHaveBeenCalledOnce();
});

it("retains receipt conflict detection across same-revision index refreshes", async () => {
  let snapshot = index(1);
  snapshot.eventGraphs = [{ path: eventPath, name: "Created", type: "event_graph", revision: 0 }];
  const dependencies = setup({
    loadProjectIndex: vi.fn(async () => projectIndexSnapshotFixture(snapshot)),
  });
  const accepted = receipt(1);
  await coordinator.submit({ result: accepted });
  await coordinator.refreshIndex();
  await expect(
    coordinator.submit({ result: receipt(1, "events/Conflict.yssbi-event") }),
  ).rejects.toMatchObject({
    code: "publication_protocol_error",
  });
  expect((await coordinator.submit({ result: accepted })).status).toBe("duplicate");
  snapshot = { ...snapshot, publicationRevision: 2 };
  await coordinator.refreshIndex();
  expect((await coordinator.submit({ result: receipt(2) })).status).toBe("duplicate");
  expect(dependencies.commitSnapshot).toHaveBeenCalledOnce();
  expect(coordinator.capturePublicationRevision()).toBe(2);
});

it("includes move receipts delivered while another graph session is being prepared", async () => {
  const source = "events/Old.yssbi-event";
  const target = "events/New.yssbi-event";
  const snapshot = index(2);
  snapshot.eventGraphs = [eventPath, target].map((path) => ({
    path,
    name: path,
    type: "event_graph",
    revision: 1,
  }));
  const pending = deferred<ReturnType<typeof makeGraphEditorSession>>();
  const prepareGraphSession = vi.fn(async (path: string) =>
    path === eventPath
      ? pending.promise
      : makeGraphEditorSession(makeEditorProjectionFixture({ graphPath: path }).projection),
  );
  const dependencies = setup({
    loadProjectIndex: vi.fn(async () => projectIndexSnapshotFixture(snapshot)),
    prepareGraphSession,
  });
  for (const path of [eventPath, source])
    installGraphProjectionFixture(
      path,
      makeEditorProjectionFixture({ graphPath: path }).projection,
    );
  const first = coordinator.submit({ result: receipt(1) });
  await vi.waitFor(() => expect(prepareGraphSession).toHaveBeenCalledOnce());
  const moved = receipt(2, target);
  moved.moves = [{ from: source, to: target, name: "New", kind: "event_graph" }];
  const second = coordinator.submit({ result: moved });
  pending.resolve(
    makeGraphEditorSession(makeEditorProjectionFixture({ graphPath: eventPath }).projection),
  );
  await Promise.all([first, second]);
  expect(dependencies.commitSnapshot).toHaveBeenCalledOnce();
  expect(vi.mocked(dependencies.commitSnapshot).mock.calls[0][0].pathRemaps.get(source)).toBe(
    target,
  );
  expect(useResourceStore.getState().graphEntities[target]).toBeDefined();
  expect(useResourceStore.getState().graphEntities[source]).toBeUndefined();
  for (const path of [eventPath, target]) {
    const graph = useResourceStore.getState();
    expect(graph.resultStates[path]).toEqual(
      makeGraphEditorSession(graph.sessions[path].projection).resultState,
    );
  }
  expect(useResourceStore.getState().resultStates[source]).toBeUndefined();
});

it("discards a delayed snapshot after project replacement", async () => {
  const pending = deferred<ProjectIndexRow>();
  const dependencies = setup({
    loadProjectIndex: vi.fn(() => pending.promise.then(projectIndexSnapshotFixture)),
  });
  const refresh = coordinator.refreshIndex();
  const rejected = expect(refresh).rejects.toMatchObject({ code: "stale_project_lifecycle" });
  await Promise.resolve();
  coordinator.startProject("project-b", 0);
  pending.resolve(index(1));
  await rejected;
  expect(dependencies.commitSnapshot).not.toHaveBeenCalled();
  expect(useResourceStore.getState().graphOrder).toEqual([]);
});

it("retains successor publications when synchronous revision and panel observers replace the project", async () => {
  for (const stage of ["receipt", "index", "panels", "catalog", "snapshot"] as const) {
    useResourceStore.getState().clear();
    const successorIndex = deferred<ProjectIndexRow>();
    const dependencies = setup();
    await coordinator.refreshIndex();
    const next = index(1);
    if (stage === "catalog" || stage === "snapshot")
      next.eventGraphs = [{ path: eventPath, name: "Created", type: "event_graph", revision: 0 }];
    vi.mocked(dependencies.loadProjectIndex).mockImplementation((projectInstanceId) =>
      projectInstanceId === "project-a"
        ? Promise.resolve(projectIndexSnapshotFixture(next))
        : successorIndex.promise.then(projectIndexSnapshotFixture),
    );
    let successorOutcome: unknown;
    let stop = () => {};
    const previousFocus = useGraphSessionStore.getState().focusedSession;
    const previousViewports = useViewportStore.getState().viewports;
    const successorFocus = { groupId: "successor-pane", graphPath: eventPath };
    const successorViewports = {
      [viewportScopeKey(successorFocus)]: { x: 41, y: 73, scale: 2 },
    };
    const replace = () => {
      stop();
      coordinator.startProject("project-b", 0);
      useResourceStore.getState().setSnapshot({ resources: [], publicationRevision: 0 });
      if (stage === "snapshot") {
        useGraphSessionStore.setState({ focusedSession: successorFocus });
        useViewportStore.setState({ viewports: successorViewports });
      }
      void coordinator.submit({ result: { ...receipt(1), projectInstanceId: "project-b" } }).then(
        (outcome) => {
          successorOutcome = outcome;
        },
        (error: unknown) => {
          successorOutcome = error;
        },
      );
    };
    stop =
      stage === "panels"
        ? useSidebarStore.subscribe((state) => {
            if (state.panels.project?.snapshot?.document.publicationRevision === 1) replace();
          })
        : stage === "catalog"
          ? useNodeCatalogStore.subscribe((state) => {
              if (state.projectWatermarks["project-a"] === 1) replace();
            })
          : useResourceStore.subscribe((state) => {
              if (state.indexRevision === 1) replace();
            });
    try {
      const pending =
        stage === "receipt" || stage === "catalog"
          ? coordinator.submit({
              result: { ...receipt(1), ...(stage === "receipt" ? { deltas: [] } : {}) },
            })
          : coordinator.refreshIndex();
      const outcome = await pending.catch((error: unknown) => error);
      expect.soft(outcome, stage).toMatchObject({ code: "stale_project_lifecycle" });
      expect.soft(coordinator.capturePublicationRevision(), stage).toBe(0);
      expect.soft(coordinator.getSnapshotForTests().pendingRevisions, stage).toEqual([1]);
      expect
        .soft(useNodeCatalogStore.getState().projectWatermarks, stage)
        .not.toHaveProperty("project-a");
      if (stage === "snapshot") {
        expect.soft(useGraphSessionStore.getState().focusedSession).toBe(successorFocus);
        expect.soft(useViewportStore.getState().viewports).toBe(successorViewports);
      }
      successorIndex.resolve({ ...index(1), projectInstanceId: "project-b" });
      await vi.waitFor(() => expect(successorOutcome, stage).toMatchObject({ status: "applied" }));
      expect(coordinator.capturePublicationRevision()).toBe(1);
    } finally {
      stop();
      successorIndex.resolve({ ...index(1), projectInstanceId: "project-b" });
      useGraphSessionStore.setState({ focusedSession: previousFocus });
      useViewportStore.setState({ viewports: previousViewports });
    }
  }
});
