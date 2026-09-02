import { beforeEach, describe, expect, it, vi } from "vitest";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  buildGraphResourceMeta,
  getDocumentState,
  markResourceLoaded,
  resourceKey,
  useResourceStore,
} from "@/features/core/resource";
import { useDocumentStateStore } from "@/features/core/resource/documentStateStore";
import { GraphProjectionService } from "@/services/nodeSystem/graphProjectionService";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import * as coordinator from "./graphProjectionCoordinator";
import { logger } from "@/features/application/observability/appLogger";

vi.mock("@/services/nodeSystem/graphProjectionService", () => ({
  GraphProjectionService: {
    loadGraph: vi.fn(),
    hydrateGraph: vi.fn(),
  },
}));

vi.mock("@/features/application/observability/appLogger", () => ({
  logger: {
    graph: {
      error: vi.fn(),
    },
  },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function invalidationApi() {
  const invalidate = (
    coordinator as typeof coordinator & {
      invalidateGraphProjection?: (graphPath: string) => Promise<boolean>;
    }
  ).invalidateGraphProjection;
  expect(invalidate).toBeTypeOf("function");
  return invalidate!;
}

describe("graphProjectionCoordinator invalidation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    coordinator.resetGraphProjectionCoordinator();
    useGraphProjectionStore.setState({ graphEntities: {} });
    useResourceStore.getState().clear();
    useProjectIOStore.setState({ projectInstanceId: "project-instance-1" });
    startProjectLifecycle("project-instance-1");
    useDocumentStateStore.getState().clear();
  });

  it("allocates a fresh lifecycle token for each publication recovery load", async () => {
    const graphPath = "events/Main.yssbi-event";
    const fixture = makeEditorProjectionFixture({ graphPath });
    const initialToken = coordinator.beginGraphLoadLifecycle(graphPath);
    expect(initialToken).toBeGreaterThan(1_000_000_000_000);
    vi.mocked(GraphProjectionService.loadGraph).mockResolvedValue(
      makeGraphEditorSession(fixture.projection),
    );

    await expect(
      coordinator.prepareGraphProjectionForPublication(graphPath, "project-instance-1", 1),
    ).resolves.toEqual(fixture.projection);
    await expect(
      coordinator.prepareGraphProjectionForPublication(graphPath, "project-instance-1", 1),
    ).resolves.toEqual(fixture.projection);

    expect(GraphProjectionService.loadGraph).toHaveBeenNthCalledWith(
      1,
      graphPath,
      "zh-CN",
      initialToken + 1,
      "project-instance-1",
    );
    expect(GraphProjectionService.loadGraph).toHaveBeenNthCalledWith(
      2,
      graphPath,
      "zh-CN",
      initialToken + 2,
      "project-instance-1",
    );
  });

  it("hydrates and atomically replaces an already loaded graph", async () => {
    const graphPath = "events/Main.yssbi-event";
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 1, title: "Current" });
    const refreshed = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 2,
      title: "Refreshed",
    });
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection, 1);
    useResourceStore
      .getState()
      .upsertResource(buildGraphResourceMeta("event", graphPath, "Main", { revision: 1 }));
    markResourceLoaded({ id: graphPath, kind: "event" });
    vi.mocked(GraphProjectionService.hydrateGraph).mockResolvedValue(
      makeGraphEditorSession(refreshed.projection),
    );

    const ok = await invalidationApi()(graphPath);

    expect(ok).toBe(true);
    expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledWith(
      "project-instance-1",
      graphPath,
      "zh-CN",
    );
    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 2,
      nodes: { "local-node": { title: "Refreshed" } },
    });
    expect(
      useResourceStore.getState().resources[resourceKey({ id: graphPath, kind: "event" })]
        ?.revision,
    ).toBe(2);
    expect(getDocumentState({ id: graphPath, kind: "event" })?.stale).toBe(false);
  });

  it("coalesces pending invalidations and runs one trailing hydrate", async () => {
    const graphPath = "events/Main.yssbi-event";
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 1, title: "Current" });
    const firstRefresh = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 2,
      title: "First refresh",
    });
    const trailingRefresh = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 3,
      title: "Trailing refresh",
    });
    const first = deferred<ReturnType<typeof makeGraphEditorSession>>();
    const trailing = deferred<ReturnType<typeof makeGraphEditorSession>>();
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: "event" });
    vi.mocked(GraphProjectionService.hydrateGraph)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(trailing.promise);

    const invalidate = invalidationApi();
    const initialRequest = invalidate(graphPath);
    const coalescedRequest = invalidate(graphPath);
    const alsoCoalescedRequest = invalidate(graphPath);

    expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledTimes(1);

    first.resolve(makeGraphEditorSession(firstRefresh.projection));
    await initialRequest;
    await vi.waitFor(() => {
      expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledTimes(2);
    });

    trailing.resolve(makeGraphEditorSession(trailingRefresh.projection));
    await expect(coalescedRequest).resolves.toBe(true);
    await expect(alsoCoalescedRequest).resolves.toBe(true);
    expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledTimes(2);
    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 3,
      nodes: { "local-node": { title: "Trailing refresh" } },
    });
  });

  it("logs hydrate IPC failure context while preserving the false API result", async () => {
    const graphPath = "events/Main.yssbi-event";
    const current = makeEditorProjectionFixture({ graphPath });
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: "event" });
    vi.mocked(GraphProjectionService.hydrateGraph).mockRejectedValue(new Error("offline detail"));

    await expect(invalidationApi()(graphPath)).resolves.toBe(false);

    expect(logger.graph.error).toHaveBeenCalledWith(
      `Graph projection hydrate IPC failed for '${graphPath}': offline detail`,
      "GraphProjectionCoordinator",
    );
  });

  it("logs invalid projection contract details while preserving the false API result", async () => {
    const graphPath = "events/Main.yssbi-event";
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 2 });
    const invalid = makeEditorProjectionFixture({ graphPath, sourceRevision: 3 });
    invalid.projection.connections[0].input = {
      kind: "declared",
      nodeId: "missing",
      portKey: "missing",
    };
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: "event" });
    vi.mocked(GraphProjectionService.hydrateGraph).mockResolvedValue(
      makeGraphEditorSession(invalid.projection),
    );

    await expect(invalidationApi()(graphPath)).resolves.toBe(false);

    expect(logger.graph.error).toHaveBeenCalledWith(
      `Graph projection hydrate contract invalid for '${graphPath}': projection connection 'local-connection' references a missing port`,
      "GraphProjectionCoordinator",
    );
  });

  it("marks the loaded projection stale when refresh fails", async () => {
    const graphPath = "events/Main.yssbi-event";
    const current = makeEditorProjectionFixture({ graphPath });
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: "event" });
    vi.mocked(GraphProjectionService.hydrateGraph).mockRejectedValue(new Error("offline"));

    await expect(invalidationApi()(graphPath)).resolves.toBe(false);

    expect(getDocumentState({ id: graphPath, kind: "event" })?.stale).toBe(true);
  });

  it("keeps the previous projection stale when refresh returns an invalid projection", async () => {
    const graphPath = "events/Main.yssbi-event";
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 2 });
    const invalid = makeEditorProjectionFixture({ graphPath, sourceRevision: 1 });
    invalid.projection.connections[0].input = {
      kind: "declared",
      nodeId: "missing",
      portKey: "missing",
    };
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: "event" });
    vi.mocked(GraphProjectionService.hydrateGraph).mockResolvedValue(
      makeGraphEditorSession(invalid.projection),
    );

    await expect(invalidationApi()(graphPath)).resolves.toBe(false);

    expect(getDocumentState({ id: graphPath, kind: "event" })?.stale).toBe(true);
    expect(useGraphProjectionStore.getState().graphEntities[graphPath].sourceRevision).toBe(2);
  });

  it("keeps the previous projection stale when the latest refresh has a lower revision", async () => {
    const graphPath = "events/Main.yssbi-event";
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 3, title: "Current" });
    const lowerRevision = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 2,
      title: "Lower revision",
    });
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: "event" });
    vi.mocked(GraphProjectionService.hydrateGraph).mockResolvedValue(
      makeGraphEditorSession(lowerRevision.projection),
    );

    await expect(invalidationApi()(graphPath)).resolves.toBe(false);

    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 3,
      requestGeneration: 1,
      nodes: { "local-node": { title: "Current" } },
    });
    expect(getDocumentState({ id: graphPath, kind: "event" })?.stale).toBe(true);
  });

  it("marks an uncached graph stale without hydrating it", async () => {
    const graphPath = "events/Unloaded.yssbi-event";

    await expect(invalidationApi()(graphPath)).resolves.toBe(false);

    expect(GraphProjectionService.hydrateGraph).not.toHaveBeenCalled();
    expect(getDocumentState({ id: graphPath, kind: "event" })?.stale).toBe(true);
  });

  it("does not install a delayed hydrate after project lifecycle replacement", async () => {
    const graphPath = "events/Main.yssbi-event";
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 1, title: "Current" });
    const stale = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 2,
      title: "Old project",
    });
    const replacement = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 1,
      title: "Replacement project",
    });
    const pending = deferred<ReturnType<typeof makeGraphEditorSession>>();
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: "event" });
    vi.mocked(GraphProjectionService.hydrateGraph).mockReturnValue(pending.promise);

    const request = invalidationApi()(graphPath);
    startProjectLifecycle("project-instance-2");
    useProjectIOStore.setState({ projectInstanceId: "project-instance-2" });
    useGraphProjectionStore.setState({ graphEntities: {} });
    useGraphProjectionStore.getState().replaceProjection(graphPath, replacement.projection, 1);
    const replacementGraph = useGraphProjectionStore.getState().graphEntities[graphPath];
    pending.resolve(makeGraphEditorSession(stale.projection));

    await expect(request).resolves.toBe(false);
    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toBe(replacementGraph);
    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 1,
      nodes: { "local-node": { title: "Replacement project" } },
    });
  });

  it("ignores a pending response after the coordinator is reset for another project", async () => {
    const graphPath = "events/Main.yssbi-event";
    const fixture = makeEditorProjectionFixture({ graphPath });
    const pending = deferred<ReturnType<typeof makeGraphEditorSession>>();
    vi.mocked(GraphProjectionService.loadGraph).mockReturnValue(pending.promise);

    const request = coordinator.loadGraphProjection(graphPath);
    coordinator.resetGraphProjectionCoordinator();
    pending.resolve(makeGraphEditorSession(fixture.projection));

    await expect(request).resolves.toBe(false);
    expect(useGraphProjectionStore.getState().hasGraph(graphPath)).toBe(false);
  });

  it("marks current when the latest response finds its generation already installed", async () => {
    const graphPath = "events/Main.yssbi-event";
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 1, title: "Current" });
    const installed = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 2,
      title: "Installed",
    });
    const pending = deferred<ReturnType<typeof makeGraphEditorSession>>();
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: "event" });
    vi.mocked(GraphProjectionService.hydrateGraph).mockReturnValue(pending.promise);

    const request = invalidationApi()(graphPath);
    useGraphProjectionStore.getState().replaceProjection(graphPath, installed.projection, 2);
    pending.resolve(makeGraphEditorSession(installed.projection));

    await expect(request).resolves.toBe(true);
    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 2,
      requestGeneration: 2,
      nodes: { "local-node": { title: "Installed" } },
    });
    expect(getDocumentState({ id: graphPath, kind: "event" })?.stale).toBe(false);
  });

  it("installs a newer trailing refresh after the current request completes", async () => {
    const graphPath = "events/Main.yssbi-event";
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 1, title: "Current" });
    const firstRefresh = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 2,
      title: "First response",
    });
    const newer = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 3,
      title: "Newer response",
    });
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: "event" });
    const first = deferred<ReturnType<typeof makeGraphEditorSession>>();
    const second = deferred<ReturnType<typeof makeGraphEditorSession>>();
    vi.mocked(GraphProjectionService.hydrateGraph)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);

    const invalidate = invalidationApi();
    const firstRequest = invalidate(graphPath);
    const newerRequest = invalidate(graphPath);
    first.resolve(makeGraphEditorSession(firstRefresh.projection));
    await expect(firstRequest).resolves.toBe(true);
    await vi.waitFor(() => {
      expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledTimes(2);
    });
    second.resolve(makeGraphEditorSession(newer.projection));
    await expect(newerRequest).resolves.toBe(true);

    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 3,
      nodes: { "local-node": { title: "Newer response" } },
    });
    expect(getDocumentState({ id: graphPath, kind: "event" })?.stale).toBe(false);
  });

  it("keeps the first refresh installed but stale when the trailing refresh fails", async () => {
    const graphPath = "events/Main.yssbi-event";
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 1, title: "Current" });
    const firstRefresh = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 2,
      title: "First response",
    });
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: "event" });
    const first = deferred<ReturnType<typeof makeGraphEditorSession>>();
    const trailing = deferred<ReturnType<typeof makeGraphEditorSession>>();
    vi.mocked(GraphProjectionService.hydrateGraph)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(trailing.promise);

    const invalidate = invalidationApi();
    const firstRequest = invalidate(graphPath);
    const trailingRequest = invalidate(graphPath);
    first.resolve(makeGraphEditorSession(firstRefresh.projection));
    await expect(firstRequest).resolves.toBe(true);
    await vi.waitFor(() => {
      expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledTimes(2);
    });
    trailing.reject(new Error("latest refresh failed"));
    await expect(trailingRequest).resolves.toBe(false);

    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 2,
      nodes: { "local-node": { title: "First response" } },
    });
    expect(getDocumentState({ id: graphPath, kind: "event" })?.stale).toBe(true);
  });
});
