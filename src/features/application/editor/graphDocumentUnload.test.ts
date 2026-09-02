import { beforeEach, describe, expect, it, vi } from "vitest";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import {
  buildGraphResourceMeta,
  getDocumentState,
  markResourceLoaded,
  useDocumentStateStore,
  useResourceStore,
} from "@/features/core/resource";
import { resetGraphProjectionCoordinator } from "@/features/application/editorProjection/graphProjectionCoordinator";
import * as graphProjectionCoordinator from "@/features/application/editorProjection/graphProjectionCoordinator";
import { GraphProjectionService } from "@/services/nodeSystem/graphProjectionService";
import { GraphService } from "@/services/graph/graphService";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { unloadGraphDocument } from "./graphDocumentUnload";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";

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

const beginGraphLoadLifecycleImpl = graphProjectionCoordinator.beginGraphLoadLifecycle;
const loadGraphProjectionImpl = graphProjectionCoordinator.loadGraphProjection;

describe("graph document lifecycle ownership", () => {
  const graphPath = "events/Main.yssbi-event";
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    resetGraphProjectionCoordinator();
    vi.spyOn(graphProjectionCoordinator, "beginGraphLoadLifecycle").mockImplementation(
      beginGraphLoadLifecycleImpl,
    );
    vi.spyOn(graphProjectionCoordinator, "loadGraphProjection").mockImplementation(
      loadGraphProjectionImpl,
    );
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject("project-instance-1", 0);
    useGraphProjectionStore.setState({ graphEntities: {} });
    useProjectIOStore.setState({ projectInstanceId: "project-instance-1" });
    useResourceStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().setSnapshot({
      resources: [buildGraphResourceMeta("event", graphPath, "Main")],
      graphOrder: [graphPath],
    });
    vi.mocked(GraphService.unloadProjectGraph).mockResolvedValue();
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
    await unloadGraphDocument(graphPath);
    const reopened = useProjectIOStore.getState().loadGraph(graphPath);

    expect(graphProjectionCoordinator.loadGraphProjection).toHaveBeenCalledTimes(2);
    expect(
      vi.mocked(graphProjectionCoordinator.loadGraphProjection).mock.calls[0]?.[1],
    ).toBeLessThan(
      vi.mocked(graphProjectionCoordinator.loadGraphProjection).mock.calls[1]?.[1] ?? 0,
    );

    oldLoad.resolve(makeGraphEditorSession(oldFixture.projection));
    await expect(initial).resolves.toBe(false);
    reopenedLoad.resolve(makeGraphEditorSession(reopenedFixture.projection));
    await expect(reopened).resolves.toBe(true);

    expect(
      useGraphProjectionStore.getState().graphEntities[graphPath]?.nodes["local-node"].title,
    ).toBe("Reopened load");
  });

  it("does not let an old unload completion overwrite a newer successful load", async () => {
    const current = makeEditorProjectionFixture({ graphPath, title: "Current" });
    const reopened = makeEditorProjectionFixture({ graphPath, title: "Reopened" });
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: "event" });
    const pendingUnload = deferred<void>();
    vi.mocked(GraphService.unloadProjectGraph).mockReturnValue(pendingUnload.promise);
    vi.mocked(GraphProjectionService.loadGraph).mockResolvedValue(
      makeGraphEditorSession(reopened.projection),
    );

    const unloading = unloadGraphDocument(graphPath);
    await expect(useProjectIOStore.getState().loadGraph(graphPath)).resolves.toBe(true);
    markResourceLoaded({ id: graphPath, kind: "event" });
    expect(getDocumentState({ id: graphPath, kind: "event" })?.loaded).toBe(true);

    pendingUnload.resolve();
    await unloading;

    expect(getDocumentState({ id: graphPath, kind: "event" })?.loaded).toBe(true);
    expect(
      useGraphProjectionStore.getState().graphEntities[graphPath]?.nodes["local-node"].title,
    ).toBe("Reopened");
    expect(vi.mocked(GraphService.unloadProjectGraph).mock.calls[0]?.[2]).toBe(
      "project-instance-1",
    );
    expect(vi.mocked(GraphService.unloadProjectGraph).mock.calls[0]?.[1]).toBeLessThan(
      vi.mocked(graphProjectionCoordinator.loadGraphProjection).mock.calls[0]?.[1] ?? 0,
    );
  });
});
