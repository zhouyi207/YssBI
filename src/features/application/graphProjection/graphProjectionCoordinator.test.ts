import { beforeEach, describe, expect, it } from "vitest";

import {
  beginGraphLoadLifecycle,
  invalidateGraphLifecycle,
  reserveGraphProjectionRequest,
  resetGraphProjectionLifecycle,
} from "./graphProjectionLifecycle";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import {
  acceptGraphProjectionEvent,
  acceptGraphProjectionSnapshot,
  awaitGraphProjection,
  resetGraphProjectionCoordinator,
} from "./graphProjectionCoordinator";

const graphPath = "events/Main.yssbi-event";

describe("Graph Projection Channel coordinator", () => {
  beforeEach(() => {
    resetGraphProjectionCoordinator();
    resetGraphProjectionLifecycle();
    useGraphProjectionStore.setState({ graphEntities: {} });
    clearProjectLifecycle();
    startProjectLifecycle("project-a");
  });

  it("applies a current publication without a Problems panel lifecycle", () => {
    beginGraphLoadLifecycle(graphPath);
    const identity = reserveGraphProjectionRequest(graphPath);
    const replacement = makeEditorProjectionFixture({ graphPath, sourceRevision: 3 });

    acceptGraphProjectionEvent({
      type: "projectionReplaced",
      projectInstanceId: "project-a",
      graphSessionId: identity.graphSessionId,
      graphPath,
      requestGeneration: identity.requestGeneration,
      replacement: { graphPath, projection: replacement.projection },
    });

    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 3,
      requestGeneration: identity.requestGeneration,
      diagnostics: replacement.projection.diagnostics,
    });
  });

  it("stages a draft publication until its command acknowledgement is consumed", async () => {
    beginGraphLoadLifecycle(graphPath);
    const identity = reserveGraphProjectionRequest(graphPath);
    const awaiting = awaitGraphProjection("project-a", graphPath, identity);
    const replacement = makeEditorProjectionFixture({ graphPath, sourceRevision: 3 });

    acceptGraphProjectionEvent({
      type: "projectionReplaced",
      projectInstanceId: "project-a",
      graphSessionId: identity.graphSessionId,
      graphPath,
      requestGeneration: identity.requestGeneration,
      replacement: { graphPath, projection: replacement.projection },
    });

    await expect(awaiting.promise).resolves.toEqual({
      graphPath,
      projection: replacement.projection,
    });
    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toBeUndefined();
  });

  it("restores the latest current projection from a snapshot", () => {
    beginGraphLoadLifecycle(graphPath);
    const identity = reserveGraphProjectionRequest(graphPath);
    const replacement = makeEditorProjectionFixture({ graphPath, sourceRevision: 5 });

    acceptGraphProjectionSnapshot({
      projectInstanceId: "project-a",
      streamId: "stream-a",
      projections: [
        {
          projectInstanceId: "project-a",
          graphSessionId: identity.graphSessionId,
          graphPath,
          requestGeneration: identity.requestGeneration,
          replacement: { graphPath, projection: replacement.projection },
        },
      ],
      latestGenerationByGraph: { [graphPath]: identity.requestGeneration },
    });

    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 5,
      requestGeneration: identity.requestGeneration,
    });
  });

  it("rejects a publication from a closed Graph session", () => {
    beginGraphLoadLifecycle(graphPath);
    const stale = reserveGraphProjectionRequest(graphPath);
    invalidateGraphLifecycle(graphPath);
    const replacement = makeEditorProjectionFixture({ graphPath, sourceRevision: 3 });

    acceptGraphProjectionEvent({
      type: "projectionReplaced",
      projectInstanceId: "project-a",
      graphSessionId: stale.graphSessionId,
      graphPath,
      requestGeneration: stale.requestGeneration,
      replacement: { graphPath, projection: replacement.projection },
    });

    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toBeUndefined();
  });

  it("isolates publications from a replaced project", () => {
    beginGraphLoadLifecycle(graphPath);
    const stale = reserveGraphProjectionRequest(graphPath);
    startProjectLifecycle("project-b");
    const replacement = makeEditorProjectionFixture({ graphPath, sourceRevision: 3 });

    acceptGraphProjectionEvent({
      type: "projectionReplaced",
      projectInstanceId: "project-a",
      graphSessionId: stale.graphSessionId,
      graphPath,
      requestGeneration: stale.requestGeneration,
      replacement: { graphPath, projection: replacement.projection },
    });

    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toBeUndefined();
  });
});
