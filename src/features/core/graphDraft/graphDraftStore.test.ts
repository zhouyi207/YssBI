import { afterEach, expect, it } from "vitest";
import { useGraphDraftStore } from "./graphDraftStore";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";

afterEach(() => useGraphDraftStore.getState().clear());

it("bounds retained edits and restores documents with freshly resolved projections", () => {
  const graphPath = "events/history.yssbi-event";
  const { projection } = makeEditorProjectionFixture({ graphPath });
  const initial = makeGraphEditorSession(projection);
  const nodeId = "00000000-0000-0000-0000-000000000001";
  initial.document.nodes[nodeId] = {
    id: nodeId,
    node_type: "math::constant",
    position: { x: 0, y: 0 },
    parameters: {},
    user_label: null,
  };
  const store = useGraphDraftStore.getState();
  store.install(graphPath, initial);
  for (let x = 1; x <= 60; x++) {
    const document = structuredClone(initial.document);
    document.nodes[nodeId].position.x = x;
    store.applyTransform(graphPath, { changed: true, document, projection });
  }
  const freshProjection = structuredClone(projection);
  freshProjection.basis.semanticInputHash = "f".repeat(64);
  const current = () => useGraphDraftStore.getState().sessions[graphPath];
  expect(current().undoStack).toHaveLength(50);
  expect(current().undoStack.every((entry) => Object.keys(entry).join() === "document")).toBe(true);
  for (let step = 0; step < 50; step++)
    expect(store.undo(graphPath, freshProjection)).not.toBeNull();
  expect(current().document.nodes[nodeId].position.x).toBe(10);
  expect(store.undo(graphPath, freshProjection)).toBeNull();
  expect(current().projection).toEqual(freshProjection);
  expect(store.redo(graphPath, freshProjection)).not.toBeNull();
  expect(current().document.nodes[nodeId].position.x).toBe(11);
});
