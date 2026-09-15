import { beforeEach, describe, expect, it, vi } from "vitest";
import { compileGraphDraft } from "./compileGraphDraft";
import { saveGraphDraft } from "./saveGraphDraft";
import { applyGraphDraftMutation, resetGraphDraftCoordinator } from "./graphDraftCoordinator";
import { useGraphDraftStore } from "@/features/core/graphDraft";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphDraftService } from "@/services/nodeSystem/graphDraftService";
import { GraphProjectionService } from "@/services/nodeSystem/graphProjectionService";
import {
  hydrateGraphProjection,
  resetGraphProjectionLifecycle,
} from "@/features/application/graphProjection/graphProjectionLifecycle";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import type { CompileGraphDraftDto, GraphDraftSaveDto } from "@/shared/types/domain/editorMutation";
import type { EditorGraphProjectionDto } from "@/shared/types/domain/editorProjection";

const graphPath = "events/compile.yssbi-event";
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

describe("Compile draft adoption", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    clearProjectLifecycle();
    startProjectLifecycle("compile-project");
    resetGraphDraftCoordinator();
    resetGraphProjectionLifecycle();
    useGraphDraftStore.getState().clear();
    useGraphProjectionStore.setState({ graphEntities: {} });
    const projection = makeEditorProjectionFixture({ graphPath }).projection;
    useGraphProjectionStore.getState().replaceProjection(graphPath, projection);
    useGraphDraftStore.getState().install(graphPath, makeGraphEditorSession(projection));
  });

  it("ignores a stale Compile failure and releases the next queued request", async () => {
    const pending = deferred<CompileGraphDraftDto>();
    vi.spyOn(GraphDraftService, "compile").mockReturnValueOnce(pending.promise);
    const first = compileGraphDraft(graphPath);
    await vi.waitFor(() => expect(GraphDraftService.compile).toHaveBeenCalledTimes(1));
    const current = useGraphDraftStore.getState().sessions[graphPath];
    const projection = structuredClone(current.projection);
    projection.basis.semanticInputHash = "1".repeat(64);
    useGraphDraftStore
      .getState()
      .applyTransform(graphPath, { changed: true, document: current.document, projection });
    vi.mocked(GraphDraftService.compile).mockResolvedValueOnce({
      type: "ready",
      artifactId: "b".repeat(64),
      cacheHit: false,
      projection,
    });
    const second = compileGraphDraft(graphPath);
    expect(GraphDraftService.compile).toHaveBeenCalledTimes(1);
    pending.reject(new Error("old request"));
    expect(await first).toBe(false);
    expect(await second).toBe(true);
    expect(useGraphDraftStore.getState().sessions[graphPath].compiledArtifactId).toBe(
      "b".repeat(64),
    );
  });

  it("orders Compile, edit and refresh without resolving the pre-edit document", async () => {
    const pendingCompile = deferred<CompileGraphDraftDto>();
    const pendingResolve = deferred<EditorGraphProjectionDto>();
    const current = useGraphDraftStore.getState().sessions[graphPath];
    const document = structuredClone(current.document);
    document.nodes["local-node"] = {
      id: "local-node",
      node_type: "tests.projected-node",
      position: { x: 10, y: 20 },
      parameters: {},
      user_label: "Edited",
    };
    const projection = structuredClone(current.projection);
    projection.basis.semanticInputHash = "1".repeat(64);
    vi.spyOn(GraphDraftService, "compile").mockReturnValueOnce(pendingCompile.promise);
    const transform = vi.spyOn(GraphDraftService, "transform").mockResolvedValue({
      changed: true,
      document,
      projection,
    });
    const resolve = vi.spyOn(GraphDraftService, "resolve").mockReturnValue(pendingResolve.promise);
    const hydrate = vi.spyOn(GraphProjectionService, "hydrateGraph");
    const compiling = compileGraphDraft(graphPath);
    await vi.waitFor(() => expect(GraphDraftService.compile).toHaveBeenCalledOnce());
    const editing = applyGraphDraftMutation({
      graphPath,
      mutation: { type: "moveNodes", payload: { positions: [] } },
    });
    const refreshing = hydrateGraphProjection(graphPath, "en-US");
    expect(transform).not.toHaveBeenCalled();
    expect(resolve).not.toHaveBeenCalled();
    pendingCompile.resolve({
      type: "ready",
      artifactId: "a".repeat(64),
      cacheHit: false,
      projection: current.projection,
    });
    expect(await compiling).toBe(true);
    expect((await editing).status).toBe("applied");
    await vi.waitFor(() => expect(resolve).toHaveBeenCalledOnce());
    expect(resolve.mock.calls[0]?.[3]).toEqual(document);
    expect(hydrate).not.toHaveBeenCalled();
    pendingResolve.resolve(projection);
    expect(await refreshing).toBe(true);
    expect(useGraphDraftStore.getState().sessions[graphPath].document).toEqual(document);
    expect(useGraphDraftStore.getState().sessions[graphPath].compiledArtifactId).toBeNull();
  });

  it("releases the save lock before a queued Compile after Save fails", async () => {
    const pending = deferred<GraphDraftSaveDto>();
    vi.spyOn(GraphDraftService, "save").mockReturnValueOnce(pending.promise);
    vi.spyOn(GraphDraftService, "compile").mockResolvedValueOnce({
      type: "ready",
      artifactId: "a".repeat(64),
      cacheHit: false,
      projection: useGraphDraftStore.getState().sessions[graphPath].projection,
    });
    const saving = saveGraphDraft(graphPath, "event");
    const failedSave = expect(saving).rejects.toThrow("save failed");
    await vi.waitFor(() => expect(GraphDraftService.save).toHaveBeenCalledOnce());
    const compiling = compileGraphDraft(graphPath);
    expect(GraphDraftService.compile).not.toHaveBeenCalled();
    pending.reject(new Error("save failed"));
    await failedSave;
    expect(await compiling).toBe(true);
    expect(useGraphDraftStore.getState().sessions[graphPath].saving).toBe(false);
  });

  it("rejects an invalid mutation projection before changing draft or history", async () => {
    const before = structuredClone(useGraphDraftStore.getState().sessions[graphPath]);
    const projection = structuredClone(before.projection);
    projection.nodes.push(structuredClone(projection.nodes[0]));
    await expect(
      applyGraphDraftMutation(
        { graphPath, locale: "en-US", mutation: { type: "moveNodes", payload: { positions: [] } } },
        {
          transform: async () => ({ changed: true, document: before.document, projection }),
        },
      ),
    ).rejects.toThrow("could not be installed");
    expect(useGraphDraftStore.getState().sessions[graphPath]).toEqual(before);
  });
});
