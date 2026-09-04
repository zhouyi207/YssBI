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
import { resetGraphProjectionLifecycle } from "@/features/application/graphProjection/graphProjectionLifecycle";
import { acceptGraphProjectionEvent } from "@/features/application/graphProjection/graphProjectionCoordinator";

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
    resetGraphProjectionLifecycle();
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
    const transform = vi
      .fn()
      .mockImplementation(
        (
          requestProjectInstanceId: string,
          graphSessionId: string,
          requestGraphPath: string,
          _locale: string,
          acceptedRevision: number,
          requestGeneration: number,
          operationId: string,
        ) => {
          acceptGraphProjectionEvent({
            type: "projectionReplaced",
            projectInstanceId: requestProjectInstanceId,
            graphSessionId,
            graphPath: requestGraphPath,
            requestGeneration,
            replacement: { graphPath: requestGraphPath, projection: fixture.projection },
          });
          return {
            projectInstanceId: requestProjectInstanceId,
            graphSessionId,
            graphPath: requestGraphPath,
            acceptedRevision,
            requestGeneration,
            operationId,
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
          };
        },
      );

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
    expect(useGraphDraftStore.getState().sessions[graphPath].draftRevision).toBe(1);
  });

  it("settles a no-op acknowledgement without waiting for a projection event", async () => {
    const { session } = installGraph();
    const transform = vi
      .fn()
      .mockImplementation(
        (
          requestProjectInstanceId: string,
          graphSessionId: string,
          requestGraphPath: string,
          _locale: string,
          acceptedRevision: number,
          requestGeneration: number,
          operationId: string,
        ) => ({
          projectInstanceId: requestProjectInstanceId,
          graphSessionId,
          graphPath: requestGraphPath,
          acceptedRevision,
          requestGeneration,
          operationId,
          document: session.document,
          patch: { operations: [] },
        }),
      );

    await expect(
      applyGraphDraftMutation(
        {
          graphPath,
          locale: "en-US",
          mutation: { type: "deleteNodes", payload: { nodeIds: [] } },
        },
        { transform },
      ),
    ).resolves.toMatchObject({ status: "noop" });
    expect(useGraphDraftStore.getState().sessions[graphPath].draftRevision).toBe(1);
  });

  it("does not install an early projection when the command acknowledgement is lost", async () => {
    installGraph();
    const ghost = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 4,
      title: "Unacknowledged projection",
    });
    let earlyEvent: Parameters<typeof acceptGraphProjectionEvent>[0] | undefined;
    const transform = vi
      .fn()
      .mockImplementation(
        (
          requestProjectInstanceId: string,
          graphSessionId: string,
          requestGraphPath: string,
          _locale: string,
          _acceptedRevision: number,
          requestGeneration: number,
        ) => {
          earlyEvent = {
            type: "projectionReplaced",
            projectInstanceId: requestProjectInstanceId,
            graphSessionId,
            graphPath: requestGraphPath,
            requestGeneration,
            replacement: { graphPath: requestGraphPath, projection: ghost.projection },
          };
          acceptGraphProjectionEvent(earlyEvent);
          throw new Error("response transport lost");
        },
      );

    await expect(
      applyGraphDraftMutation(
        {
          graphPath,
          locale: "en-US",
          mutation: { type: "deleteNodes", payload: { nodeIds: [] } },
        },
        { transform },
      ),
    ).rejects.toThrow("response transport lost");
    if (earlyEvent) acceptGraphProjectionEvent(earlyEvent);

    expect(
      useGraphProjectionStore.getState().graphEntities[graphPath].nodes["local-node"].display.title,
    ).not.toBe("Unacknowledged projection");
    expect(useGraphDraftStore.getState().sessions[graphPath].draftRevision).toBe(0);
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
