import { beforeEach, describe, expect, it, vi } from "vitest";

import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import {
  applyGraphDraftMutation,
  resetGraphDraftCoordinator,
} from "@/features/application/graphDraft/graphDraftCoordinator";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { isGraphDraftSaving, useGraphDraftStore } from "@/features/core/graphDraft";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  buildGraphResourceMeta,
  getDocumentState,
  markResourceLoaded,
  useDocumentStateStore,
  useResourceStore,
} from "@/features/core/resource";
import { GraphDraftService } from "@/services/nodeSystem/graphDraftService";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { saveGraphDraft } from "@/features/application/graphDraft/saveGraphDraft";

vi.mock("@/services/nodeSystem/graphDraftService", () => ({
  GraphDraftService: { save: vi.fn() },
}));

const graphPath = "events/Main.yssbi-event";
const projectInstanceId = "project-instance-1";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function installGraph() {
  const fixture = makeEditorProjectionFixture({ graphPath, sourceRevision: 4 });
  const session = makeGraphEditorSession(fixture.projection);
  useGraphProjectionStore.getState().replaceProjection(graphPath, fixture.projection, 1);
  useGraphDraftStore.getState().install(graphPath, session);
  useResourceStore
    .getState()
    .upsertResource(buildGraphResourceMeta("event", graphPath, "Main", { revision: 4 }));
  markResourceLoaded({ id: graphPath, kind: "event" });
  return { fixture, session };
}

describe("Graph draft save boundary", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetGraphDraftCoordinator();
    useGraphProjectionStore.setState({ graphEntities: {} });
    useGraphDraftStore.getState().clear();
    useResourceStore.getState().clear();
    useDocumentStateStore.getState().clear();
    clearProjectLifecycle();
    startProjectLifecycle(projectInstanceId);
    useProjectIOStore.setState({ projectInstanceId });
  });

  it("applies an editor mutation only to the frontend draft and marks it dirty", async () => {
    const { fixture, session } = installGraph();
    const transform = vi.fn().mockResolvedValue({
      document: session.document,
      patch: {
        operations: [
          {
            operation: "update_node",
            before: {
              id: "00000000-0000-0000-0000-000000000001",
              node_type: "tests.node",
              position: { x: 0, y: 0 },
              parameters: {},
              user_label: null,
            },
            after: {
              id: "00000000-0000-0000-0000-000000000001",
              node_type: "tests.node",
              position: { x: 30, y: 40 },
              parameters: {},
              user_label: null,
            },
          },
        ],
      },
      projectionReplacement: { graphPath, projection: fixture.projection },
    });

    await expect(
      applyGraphDraftMutation(
        {
          graphPath,
          locale: "en-US",
          mutation: {
            type: "moveNodes",
            payload: {
              positions: [
                {
                  nodeId: "00000000-0000-0000-0000-000000000001",
                  position: { x: 30, y: 40 },
                },
              ],
            },
          },
        },
        { transform },
      ),
    ).resolves.toMatchObject({ status: "applied" });

    expect(transform).toHaveBeenCalledOnce();
    expect(getDocumentState({ id: graphPath, kind: "event" })?.dirty).toBe(true);
    expect(useGraphProjectionStore.getState().graphEntities[graphPath].sourceRevision).toBe(4);
  });

  it("locks every new Graph edit until the overwrite save settles", async () => {
    const { fixture, session } = installGraph();
    const pending = deferred<Awaited<ReturnType<typeof GraphDraftService.save>>>();
    vi.mocked(GraphDraftService.save).mockReturnValue(pending.promise);

    const save = saveGraphDraft(graphPath, "event");
    expect(isGraphDraftSaving(graphPath)).toBe(true);
    const transform = vi.fn();
    await expect(
      applyGraphDraftMutation(
        {
          graphPath,
          locale: "en-US",
          mutation: { type: "deleteNodes", payload: { nodeIds: [] } },
        },
        { transform },
      ),
    ).resolves.toEqual({ status: "saving" });
    expect(transform).not.toHaveBeenCalled();

    pending.resolve({
      projectInstanceId,
      operationId: "00000000-0000-0000-0000-000000000010",
      document: session.document,
      projectionReplacement: { graphPath, projection: fixture.projection },
      history: { canUndo: true, canRedo: false },
    });
    await expect(save).resolves.toBe(true);
    expect(isGraphDraftSaving(graphPath)).toBe(false);
    expect(getDocumentState({ id: graphPath, kind: "event" })?.dirty).toBe(false);
  });
});
