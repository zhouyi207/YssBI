import {
  installGraphProjectionFixture,
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import {
  buildFileResourceMeta,
  getDocumentState,
  markResourceLoaded,
  resourceKey,
  useResourceStore,
} from "@/features/core/resource";
import { resetGraphProjectionLifecycle } from "@/features/application/graphProjection/graphProjectionLifecycle";
import * as graphProjectionLifecycle from "@/features/application/graphProjection/graphProjectionLifecycle";
import { GraphProjectionService } from "@/services/nodeSystem/graphProjectionService";
import { GraphService } from "@/services/graph/graphService";
import { useExecutionStore } from "@/features/core/execution";
import {
  editorViewportScope,
  getViewport,
  setViewportLive,
  useViewportStore,
} from "@/features/core/viewport";

import { unloadGraphDocument } from "./graphDocumentUnload";
import {
  enforceGraphDocumentCacheLimit,
  MAX_HYDRATED_GRAPH_DOCUMENTS,
} from "./graphDocumentCachePolicy";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";

vi.mock("@/features/application/graphProjection/graphActivity", () => ({
  ensureGraphActivity: async () => undefined,
  resetGraphActivity: () => undefined,
}));

vi.mock("@/features/application/editor/graphDocumentRetention", () => ({
  shouldRetainGraphDocument: () => false,
}));

vi.mock("@/services/nodeSystem/graphProjectionService", () => ({
  GraphProjectionService: {
    loadGraph: vi.fn(),
    hydrateGraph: vi.fn(),
  },
}));

vi.mock("@/services/graph/graphService", () => ({
  GraphService: {
    unloadProjectGraph: vi.fn(),
  },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

const beginGraphLoadLifecycleImpl = graphProjectionLifecycle.beginGraphLoadLifecycle;
const loadGraphProjectionImpl = graphProjectionLifecycle.loadGraphProjection;

describe("graph document lifecycle ownership", () => {
  const graphPath = "events/Main.yssbi-event";
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    resetGraphProjectionLifecycle();
    vi.spyOn(graphProjectionLifecycle, "beginGraphLoadLifecycle").mockImplementation(
      beginGraphLoadLifecycleImpl,
    );
    vi.spyOn(graphProjectionLifecycle, "loadGraphProjection").mockImplementation(
      loadGraphProjectionImpl,
    );
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject("project-instance-1", 0);
    useResourceStore.getState().clear();
    useProjectIOStore.setState({ projectInstanceId: "project-instance-1" });
    useResourceStore.getState().clear();
    useResourceStore.getState().setSnapshot({
      resources: [buildFileResourceMeta("event_graph", graphPath, "Main")],
      graphOrder: [graphPath],
    });
    vi.mocked(GraphService.unloadProjectGraph).mockResolvedValue(true);
  });

  it("starts a new load when an initial pending load is unloaded and immediately reopened", async () => {
    const oldFixture = makeEditorProjectionFixture({ graphPath, title: "Old load" });
    const reopenedFixture = makeEditorProjectionFixture({ graphPath, title: "Reopened load" });
    const oldLoad = deferred<ReturnType<typeof makeGraphEditorSession>>();
    const reopenedLoad = deferred<ReturnType<typeof makeGraphEditorSession>>();
    vi.mocked(GraphProjectionService.loadGraph)
      .mockReturnValueOnce(oldLoad.promise)
      .mockReturnValueOnce(reopenedLoad.promise);

    const initial = useProjectIOStore.getState().loadGraph(graphPath);
    await vi.waitFor(() => expect(GraphProjectionService.loadGraph).toHaveBeenCalledOnce());
    const unloading = unloadGraphDocument(graphPath);
    oldLoad.resolve(makeGraphEditorSession(oldFixture.projection));
    await unloading;
    const reopened = useProjectIOStore.getState().loadGraph(graphPath);

    expect(graphProjectionLifecycle.loadGraphProjection).toHaveBeenCalledTimes(2);
    expect(vi.mocked(graphProjectionLifecycle.loadGraphProjection).mock.calls[0]?.[1]).toBeLessThan(
      vi.mocked(graphProjectionLifecycle.loadGraphProjection).mock.calls[1]?.[1] ?? 0,
    );

    await expect(initial).resolves.toBe(false);
    reopenedLoad.resolve(makeGraphEditorSession(reopenedFixture.projection));
    await expect(reopened).resolves.toBe(true);

    expect(
      useResourceStore.getState().graphEntities[graphPath]?.nodes["local-node"].display.title,
    ).toBe("Reopened load");
  });

  it("keeps the loaded frame until Rust confirms unloading, then removes its flags atomically", async () => {
    const ref = { id: graphPath, kind: "event_graph" as const };
    const key = resourceKey(ref);
    installGraphProjectionFixture(graphPath, makeEditorProjectionFixture({ graphPath }).projection);
    markResourceLoaded(ref);
    const pending = deferred<boolean>();
    vi.mocked(GraphService.unloadProjectGraph).mockReturnValueOnce(pending.promise);
    const observed: unknown[] = [];
    const stop = useResourceStore.subscribe((state) => {
      observed.push({
        loaded: state.resources[key].loaded,
        hasDocument: !!state.documents[key],
        hasGraph: !!state.sessions[graphPath],
      });
    });
    try {
      const unloading = unloadGraphDocument(graphPath);
      await vi.waitFor(() => expect(GraphService.unloadProjectGraph).toHaveBeenCalledOnce());
      expect.soft(observed).toEqual([]);
      expect.soft(getDocumentState(ref)?.loaded).toBe(true);
      pending.resolve(false);
      await unloading;
      expect(observed).toEqual([]);
      await expect(useProjectIOStore.getState().loadGraph(graphPath)).resolves.toBe(true);
      expect(GraphProjectionService.loadGraph).not.toHaveBeenCalled();
      await unloadGraphDocument(graphPath);
      expect(observed).toEqual([{ loaded: false, hasDocument: false, hasGraph: false }]);
    } finally {
      stop();
    }
  });

  it("does not let an old unload completion overwrite a newer successful load", async () => {
    const current = makeEditorProjectionFixture({ graphPath, title: "Current" });
    const reopened = makeEditorProjectionFixture({ graphPath, title: "Reopened" });
    installGraphProjectionFixture(graphPath, current.projection);
    markResourceLoaded({ id: graphPath, kind: "event_graph" });
    const pendingUnload = deferred<boolean>();
    vi.mocked(GraphService.unloadProjectGraph).mockReturnValue(pendingUnload.promise);
    vi.mocked(GraphProjectionService.loadGraph).mockResolvedValue(
      makeGraphEditorSession(reopened.projection),
    );

    const unloading = unloadGraphDocument(graphPath);
    await vi.waitFor(() => expect(GraphService.unloadProjectGraph).toHaveBeenCalledOnce());
    const loading = useProjectIOStore.getState().loadGraph(graphPath);
    pendingUnload.resolve(true);
    await expect(loading).resolves.toBe(true);
    markResourceLoaded({ id: graphPath, kind: "event_graph" });
    expect(getDocumentState({ id: graphPath, kind: "event_graph" })?.loaded).toBe(true);

    await unloading;

    expect(getDocumentState({ id: graphPath, kind: "event_graph" })?.loaded).toBe(true);
    expect(
      useResourceStore.getState().graphEntities[graphPath]?.nodes["local-node"].display.title,
    ).toBe("Reopened");
    expect(vi.mocked(GraphService.unloadProjectGraph).mock.calls[0]?.[2]).toBe(
      "project-instance-1",
    );
    expect(vi.mocked(GraphService.unloadProjectGraph).mock.calls[0]?.[1]).toBeLessThan(
      vi.mocked(graphProjectionLifecycle.loadGraphProjection).mock.calls[0]?.[1] ?? 0,
    );
  });

  it("stops a cache eviction batch when the project changes during an unload", async () => {
    const paths = Array.from(
      { length: MAX_HYDRATED_GRAPH_DOCUMENTS + 1 },
      (_, index) => `events/Cached-${index}.yssbi-event`,
    );
    const installGraphs = (title: string) => {
      useResourceStore.getState().clear();
      useResourceStore.getState().setSnapshot({
        resources: paths.map((path) => buildFileResourceMeta("event_graph", path, title)),
        graphOrder: paths,
      });
      for (const path of paths) {
        installGraphProjectionFixture(
          path,
          makeEditorProjectionFixture({ graphPath: path, title }).projection,
        );
        markResourceLoaded({ id: path, kind: "event_graph" });
      }
    };
    installGraphs("Original");
    const pending = deferred<boolean>();
    vi.mocked(GraphService.unloadProjectGraph).mockReturnValueOnce(pending.promise);
    const evicting = enforceGraphDocumentCacheLimit();
    await vi.waitFor(() => expect(GraphService.unloadProjectGraph).toHaveBeenCalledOnce());

    projectPublicationCoordinator.startProject("project-successor", 0);
    useProjectIOStore.setState({ projectInstanceId: "project-successor" });
    installGraphs("Successor");
    const successorGraphs = useResourceStore.getState().graphEntities;
    pending.resolve(true);
    await evicting;

    expect.soft(GraphService.unloadProjectGraph).toHaveBeenCalledOnce();
    expect.soft(useResourceStore.getState().graphEntities).toBe(successorGraphs);

    await enforceGraphDocumentCacheLimit();
    expect(Object.keys(useResourceStore.getState().graphEntities)).toHaveLength(
      MAX_HYDRATED_GRAPH_DOCUMENTS,
    );
    expect(vi.mocked(GraphService.unloadProjectGraph).mock.lastCall?.[2]).toBe("project-successor");
  });

  it("stops unload cleanup when session removal observers replace the project or reopen the graph", async () => {
    const scope = editorViewportScope("reopened", graphPath);
    const viewport = { x: 70, y: 80, scale: 2 };
    for (const replacement of ["project", "graph"] as const) {
      projectPublicationCoordinator.startProject("project-instance-1", 0);
      useExecutionStore.setState({ graphs: {} });
      useViewportStore.getState().clear();
      installGraphProjectionFixture(
        graphPath,
        makeEditorProjectionFixture({ graphPath }).projection,
      );
      vi.mocked(GraphProjectionService.loadGraph).mockResolvedValue(
        makeGraphEditorSession(
          makeEditorProjectionFixture({ graphPath, title: "Reopened" }).projection,
        ),
      );
      let reopened: Promise<boolean> | undefined;
      const stop = useResourceStore.subscribe((state) => {
        if (state.sessions[graphPath]) return;
        stop();
        if (replacement === "project")
          projectPublicationCoordinator.startProject("project-successor", 0);
        else reopened = useProjectIOStore.getState().loadGraph(graphPath);
        useExecutionStore.getState().submitExecution(graphPath);
        setViewportLive(scope, viewport);
      });
      try {
        await unloadGraphDocument(graphPath);
        if (reopened) expect(await reopened).toBe(true);
        expect
          .soft(useExecutionStore.getState().graphs[graphPath]?.status, replacement)
          .toBe("submitting");
        expect.soft(getViewport(scope), replacement).toEqual(viewport);
      } finally {
        stop();
        useExecutionStore.setState({ graphs: {} });
        useViewportStore.getState().clear();
      }
    }
  });

  it("stops graph loading when loading-status observers replace the project", async () => {
    const successorPaths = Array.from({ length: MAX_HYDRATED_GRAPH_DOCUMENTS + 1 }, (_, index) =>
      index === 0 ? graphPath : `events/Successor-${index}.yssbi-event`,
    );
    for (const status of ["loading", "ready"] as const) {
      projectPublicationCoordinator.startProject("project-instance-1", 0);
      useResourceStore.getState().clear();
      useResourceStore.getState().setSnapshot({
        resources: [buildFileResourceMeta("event_graph", graphPath, "Original")],
        graphOrder: [graphPath],
      });
      useProjectIOStore.setState({
        projectInstanceId: "project-instance-1",
        graphLoadStatus: {},
      });
      vi.mocked(GraphProjectionService.loadGraph)
        .mockClear()
        .mockResolvedValue(
          makeGraphEditorSession(makeEditorProjectionFixture({ graphPath }).projection),
        );
      vi.mocked(GraphService.unloadProjectGraph).mockClear();
      let replaced = false;
      const stop = useProjectIOStore.subscribe((state) => {
        if (state.graphLoadStatus[graphPath] !== status) return;
        stop();
        replaced = true;
        projectPublicationCoordinator.startProject("project-successor", 0);
        useProjectIOStore.setState({
          projectInstanceId: "project-successor",
          graphLoadStatus: {},
        });
        useResourceStore.getState().clear();
        useResourceStore.getState().setSnapshot({
          resources: successorPaths.map((path) =>
            buildFileResourceMeta("event_graph", path, "Successor"),
          ),
          graphOrder: successorPaths,
        });
        for (const path of successorPaths) {
          installGraphProjectionFixture(
            path,
            makeEditorProjectionFixture({ graphPath: path, title: "Successor" }).projection,
          );
          markResourceLoaded({ id: path, kind: "event_graph" });
        }
      });
      try {
        expect.soft(await useProjectIOStore.getState().loadGraph(graphPath), status).toBe(false);
        expect.soft(replaced, status).toBe(true);
        expect.soft(GraphService.unloadProjectGraph, status).not.toHaveBeenCalled();
        expect
          .soft(Object.keys(useResourceStore.getState().graphEntities), status)
          .toEqual(successorPaths);
        expect.soft(useProjectIOStore.getState().graphLoadStatus, status).toEqual({});
        if (status === "loading") {
          expect.soft(GraphProjectionService.loadGraph).not.toHaveBeenCalled();
        }
      } finally {
        stop();
      }
    }
  });
});
