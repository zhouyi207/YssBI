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
import {
  useResourceStore,
  useDocumentStateStore,
  buildFileResourceMeta,
  markResourceLoaded,
  markResourceDirty,
  resourceKey,
} from "@/features/core/resource";
import { useDocProjectionStore } from "@/features/core/resource/docProjectionStore";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useGraphMetaStore } from "@/features/core/dataStore/graphMetaStore";
import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";

import { captureProjectLifecycleState } from "@/features/core/projectLifecycle/projectLifecycleAuthority";

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
    prepareGraphSession: vi.fn(async (path: string) =>
      makeGraphEditorSession(makeEditorProjectionFixture({ graphPath: path }).projection),
    ),
    captureLoadedGraphPaths: () =>
      new Set(Object.keys(useGraphProjectionStore.getState().graphEntities)),
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
  useDocumentStateStore.setState({ documents: {} });
  useGraphProjectionStore.getState().clear();
  useGraphMetaStore.setState({ graphs: {} });
  useChartDocumentStore.getState().clear();
  useDatabaseStore.setState({ databases: {}, revisions: {} });
});
afterEach(() => coordinator?.cancelProject());

it("applies late delete authorization to retained dirty content after an index-only refresh", async () => {
  const dependencies = setup({
    loadProjectIndex: vi.fn(async () => projectIndexSnapshotFixture(index(1))),
  });
  const path = "docs/Report.md";
  const ref = { id: path, kind: "doc" as const };
  useResourceStore
    .getState()
    .upsertResource(buildFileResourceMeta("doc", path, "Report", { revision: 0 }));
  markResourceLoaded(ref);
  markResourceDirty(ref, true);
  useDocProjectionStore.getState().install({
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
  expect(useDocProjectionStore.getState().documents[path]).toBeUndefined();
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
  expect(useGraphProjectionStore.getState().graphEntities[target]).toBeDefined();
  expect(useGraphProjectionStore.getState().graphEntities[source]).toBeUndefined();
  for (const path of [eventPath, target]) {
    const graph = useGraphProjectionStore.getState();
    expect(graph.resultStates[path]).toEqual(
      makeGraphEditorSession(graph.sessions[path].projection).resultState,
    );
  }
  expect(useGraphProjectionStore.getState().resultStates[source]).toBeUndefined();
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
