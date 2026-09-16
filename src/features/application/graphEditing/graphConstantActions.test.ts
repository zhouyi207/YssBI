import { beforeEach, expect, it, vi } from "vitest";
import { updateGraphConstant } from "./graphConstantActions";
import { resetGraphEditCoordinator } from "./graphEditCoordinator";
import { useGraphEditingStore } from "@/features/core/graphEditing";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphEditingService } from "@/services/nodeSystem/graphEditingService";
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
  resetGraphEditCoordinator();
  useGraphEditingStore.getState().clear();
  useGraphProjectionStore.setState({ graphEntities: {} });
  const projection = makeEditorProjectionFixture({ graphPath }).projection;
  useGraphProjectionStore.getState().replaceProjection(graphPath, projection);
  const session = makeGraphEditorSession(projection);
  session.document.constants = {
    [id]: {
      id,
      name: "Before",
      dataType: { kind: "Scalar", inner: "Numeric" },
      dataValue: { Int64: 0 },
    },
  };
  useGraphEditingStore.getState().install(graphPath, session);
});

it("merges queued constant edits against the preceding Rust result", async () => {
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  const transform = vi
    .spyOn(GraphEditingService, "transform")
    .mockImplementation(async (_project, _path, _locale, version, mutation) => {
      await pending;
      if (mutation.type !== "setConstant" || !mutation.payload.constant)
        throw new Error("expected constant edit");
      const next = structuredClone(useGraphEditingStore.getState().sessions[graphPath].document);
      next.constants![id] = mutation.payload.constant;
      return {
        changed: true,
        editing: {
          version: { ...version, revision: String(BigInt(version.revision) + 1n) },
          dirty: true,
          canUndo: true,
          canRedo: false,
        },
        document: next,
        projection: makeEditorProjectionFixture({ graphPath }).projection,
      };
    });
  const rename = updateGraphConstant(graphPath, id, { name: "After" });
  const value = updateGraphConstant(graphPath, id, { dataValue: { kind: "Float64", value: 7 } });
  await vi.waitFor(() => expect(transform).toHaveBeenCalledTimes(1));
  release();
  const outcomes = await Promise.all([rename, value]);
  expect(outcomes.map((outcome) => outcome.status)).toEqual(["applied", "applied"]);
  const session = useGraphEditingStore.getState().sessions[graphPath];
  expect(session.document.constants![id]).toMatchObject({ name: "After", dataValue: { Int64: 7 } });
  expect(session.version.revision).toBe("2");
  expect(session.canUndo).toBe(true);
  expect(session.saveDirty).toBe(true);
});
