import { beforeEach, expect, it, vi } from "vitest";
import { updateGraphConstant } from "./graphConstantActions";
import { resetGraphDraftCoordinator } from "./graphDraftCoordinator";
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

const graphPath = "events/constants.yssbi-event";
const id = "00000000-0000-0000-0000-000000000004";

beforeEach(() => {
  vi.restoreAllMocks();
  clearProjectLifecycle();
  startProjectLifecycle("constant-project");
  resetGraphDraftCoordinator();
  useGraphDraftStore.getState().clear();
  useGraphProjectionStore.setState({ graphEntities: {} });
  const projection = makeEditorProjectionFixture({ graphPath }).projection;
  useGraphProjectionStore.getState().replaceProjection(graphPath, projection);
  const session = makeGraphEditorSession(projection);
  session.document.constants = {
    [id]: { id, name: "Before", dataType: { kind: "Int64" }, dataValue: { Int64: 0 } },
  };
  useGraphDraftStore.getState().install(graphPath, session);
});

it("merges queued constant edits against the preceding Rust result", async () => {
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  const transform = vi
    .spyOn(GraphDraftService, "transform")
    .mockImplementation(async (_project, _path, _locale, document, mutation) => {
      await pending;
      if (mutation.type !== "setConstant" || !mutation.payload.constant)
        throw new Error("expected constant edit");
      const next = structuredClone(document);
      next.constants![id] = mutation.payload.constant;
      return {
        changed: true,
        document: next,
        projection: makeEditorProjectionFixture({ graphPath }).projection,
      };
    });
  const rename = updateGraphConstant(graphPath, id, { name: "After" });
  const value = updateGraphConstant(graphPath, id, { dataValue: { kind: "Int64", value: 7 } });
  expect(transform).toHaveBeenCalledTimes(1);
  release();
  const outcomes = await Promise.all([rename, value]);
  expect(outcomes.map((outcome) => outcome.status)).toEqual(["applied", "applied"]);
  const session = useGraphDraftStore.getState().sessions[graphPath];
  expect(session.document.constants![id]).toMatchObject({ name: "After", dataValue: { Int64: 7 } });
  expect(session.undoStack).toHaveLength(2);
  expect(session.saveDirty).toBe(true);
});
