import { beforeEach, describe, expect, it } from "vitest";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import {
  completePendingMutation,
  registerPendingMutation,
  resetPendingMutations,
} from "@/features/application/editorMutation/pendingMutationRegistry";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import {
  captureRevisionedProjectCommandSnapshot,
  captureSettledGraphSaveCommandContext,
} from "./projectCommandContext";

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const replacementProjectInstanceId = "00000000-0000-0000-0000-000000000602";

describe("captureRevisionedProjectCommandSnapshot", () => {
  beforeEach(() => {
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 3);
    resetPendingMutations();
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it("returns authority with the lifecycle captured before the synchronous read", () => {
    const snapshot = captureRevisionedProjectCommandSnapshot(() => ({ revision: 7 }));

    expect(snapshot.authority).toEqual({ revision: 7 });
    expect(snapshot.context).toMatchObject({
      projectInstanceId,
      publicationRevision: 3,
    });
    expect(snapshot.context.isCurrent()).toBe(true);
  });

  it("rejects the snapshot when the authority reader replaces the project lifecycle", () => {
    expect(() =>
      captureRevisionedProjectCommandSnapshot(() => {
        projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
        return { revision: 7 };
      }),
    ).toThrow(expect.objectContaining({ code: "stale_project_lifecycle" }));
  });

  it("captures the revision installed after an in-flight graph mutation settles", async () => {
    const graphPath = "events/Main.yssbi-event";
    useGraphDataStore
      .getState()
      .replaceProjection(
        graphPath,
        makeEditorProjectionFixture({ graphPath, sourceRevision: 1 }).projection,
        1,
      );
    registerPendingMutation({
      operationId: "mutation-1",
      graphPath,
      baseRevision: 1,
    });

    const capture = captureSettledGraphSaveCommandContext(graphPath);
    let settled = false;
    void capture.then(() => {
      settled = true;
    });
    await Promise.resolve();
    expect(settled).toBe(false);

    useGraphDataStore
      .getState()
      .replaceProjection(
        graphPath,
        makeEditorProjectionFixture({ graphPath, sourceRevision: 2 }).projection,
        2,
      );
    completePendingMutation("mutation-1");

    await expect(capture).resolves.toMatchObject({
      projectInstanceId,
      expectedRevision: 2,
    });
  });
});
