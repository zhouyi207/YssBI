import { beforeEach, describe, expect, it, vi } from "vitest";
import { compileGraphDraft } from "./compileGraphDraft";
import { applyGraphDraftMutation, resetGraphDraftCoordinator } from "./graphDraftCoordinator";
import { useGraphDraftStore } from "@/features/core/graphDraft";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphDraftService } from "@/services/nodeSystem/graphDraftService";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import type { CompileGraphDraftDto } from "@/shared/types/domain/editorMutation";

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
    useGraphDraftStore.getState().clear();
    useGraphProjectionStore.setState({ graphEntities: {} });
    const projection = makeEditorProjectionFixture({ graphPath }).projection;
    useGraphProjectionStore.getState().replaceProjection(graphPath, projection);
    useGraphDraftStore.getState().install(graphPath, makeGraphEditorSession(projection));
  });

  it("adopts Ready and Blocked without replacing the draft or its save history", async () => {
    const before = structuredClone(useGraphDraftStore.getState().sessions[graphPath]);
    vi.spyOn(GraphDraftService, "compile").mockResolvedValue({
      type: "ready",
      artifactId: "a".repeat(64),
      cacheHit: false,
      projection: before.projection,
    });
    expect(await compileGraphDraft(graphPath)).toBe(true);
    const blocked = structuredClone(before.projection);
    blocked.outcome = { type: "analysisBlocked" };
    blocked.diagnostics = [
      {
        code: "compiler.input.unbound",
        messageKey: "diagnostics.compiler.input.unbound",
        arguments: { port: "Input" },
        severity: "warning",
        blocking: true,
        location: { kind: "graph" },
        related: [],
      },
    ];
    blocked.hasBlockingDiagnostics = true;
    vi.mocked(GraphDraftService.compile).mockResolvedValue({
      type: "blocked",
      projection: blocked,
    });
    expect(await compileGraphDraft(graphPath)).toBe(false);
    const after = useGraphDraftStore.getState().sessions[graphPath];
    expect(after.document).toEqual(before.document);
    expect(after.savedDocument).toEqual(before.savedDocument);
    expect(after.undoStack).toEqual(before.undoStack);
    expect(after.saveDirty).toBe(false);
    expect(after.compileStatus).toBe("blocked");
    expect(after.compiledArtifactId).toBeNull();
    expect(
      useGraphProjectionStore.getState().graphEntities[graphPath].diagnostics[0].blocking,
    ).toBe(true);
  });

  it("ignores an old Compile failure after an edit and a newer successful request", async () => {
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
    expect(await compileGraphDraft(graphPath)).toBe(true);
    pending.reject(new Error("old request"));
    expect(await first).toBe(false);
    expect(useGraphDraftStore.getState().sessions[graphPath].compiledArtifactId).toBe(
      "b".repeat(64),
    );
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
