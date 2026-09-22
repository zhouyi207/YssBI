import { afterEach, expect, it } from "vitest";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { portAddressKey } from "@/features/domain/editorProjection";
import { useGraphProjectionStore } from "./graphProjectionStore";

const graphPath = "events/Atomic.yssbi-event";

function frame() {
  const parts = [1, 2, 3].map((id) =>
    makeEditorProjectionFixture({
      graphPath,
      nodeId: `00000000-0000-0000-0000-${String(id).padStart(12, "0")}`,
    }),
  );
  const projection = {
    ...parts[0].projection,
    nodes: parts.flatMap((part) => part.projection.nodes),
  };
  const session = makeGraphEditorSession(projection);
  session.resultState = {
    ...session.resultState,
    revision: "1",
    outputs: session.resultState.outputs.map((entry, i) => ({
      ...entry,
      state: "valid",
      resultId: String(i + 1),
    })),
  };
  return { session, parts };
}

afterEach(() => useGraphProjectionStore.getState().clear());

it("publishes connection edits with matching results once and retains unrelated snapshot entities", () => {
  const { session, parts } = frame();
  useGraphProjectionStore.getState().install(graphPath, session);
  const before = useGraphProjectionStore.getState();
  const next = structuredClone(session);
  next.editing.version.revision = "1";
  next.editing.dirty = true;
  next.projection.basis.semanticInputHash = "1".repeat(64);
  next.projection.connections = [
    {
      connectionId: "00000000-0000-0000-0000-000000000010",
      output: parts[0].outputAddress,
      input: parts[1].inputAddress,
      order: null,
    },
  ];
  const output = next.projection.nodes[0].ports.find((port) => port.direction === "output")!;
  const input = next.projection.nodes[1].ports.find((port) => port.direction === "input")!;
  output.connections = { ...output.connections, current: 1, canMove: true };
  input.connections = {
    ...input.connections,
    current: 1,
    canAppend: false,
    canReplace: true,
    canMove: true,
  };
  next.resultState = {
    ...next.resultState,
    revision: "2",
    semanticInputHash: next.projection.basis.semanticInputHash,
    outputs: next.resultState.outputs.map((entry, i) =>
      i === 1 ? { ...entry, state: "stale", resultId: null } : entry,
    ),
    connections: [
      {
        output: { graphPath, port: parts[0].outputAddress },
        input: parts[1].inputAddress,
        state: "new",
      },
    ],
  };
  const updates: string[] = [];
  const unsubscribe = useGraphProjectionStore.subscribe((state) => {
    const hash = state.sessions[graphPath].semanticInputHash;
    expect(state.graphEntities[graphPath].basis.semanticInputHash).toBe(hash);
    expect(state.resultStates[graphPath].semanticInputHash).toBe(hash);
    updates.push(hash);
  });
  useGraphProjectionStore.getState().hydrate(graphPath, next);
  unsubscribe();
  expect(updates).toEqual([next.projection.basis.semanticInputHash]);
  const after = useGraphProjectionStore.getState();
  const unaffected = parts[2].outputAddress.nodeId;
  expect(after.graphEntities[graphPath].nodes[unaffected]).toBe(
    before.graphEntities[graphPath].nodes[unaffected],
  );
  expect(after.graphEntities[graphPath].pins[portAddressKey(parts[2].outputAddress)]).toBe(
    before.graphEntities[graphPath].pins[portAddressKey(parts[2].outputAddress)],
  );
  expect(after.resultStates[graphPath].outputs[2]).toBe(before.resultStates[graphPath].outputs[2]);
});

it("keeps newer execution results and rejects stale or inconsistent graph frames without publication", () => {
  const { session } = frame();
  const store = useGraphProjectionStore.getState();
  store.install(graphPath, session);
  store.setResultState(graphPath, { ...session.resultState, revision: "3" });
  const results = useGraphProjectionStore.getState().resultStates[graphPath];
  const next = structuredClone(session);
  next.editing.version.revision = "2";
  next.resultState = { ...next.resultState, revision: "2" };
  store.hydrate(graphPath, next);
  expect(useGraphProjectionStore.getState().resultStates[graphPath]).toBe(results);
  const sessionBeforeResults = useGraphProjectionStore.getState().sessions[graphPath];
  store.hydrate(graphPath, { ...next, resultState: { ...next.resultState, revision: "4" } });
  expect(useGraphProjectionStore.getState().sessions[graphPath]).toBe(sessionBeforeResults);
  const current = useGraphProjectionStore.getState();
  let notifications = 0;
  const unsubscribe = useGraphProjectionStore.subscribe(() => notifications++);
  store.hydrate(graphPath, session);
  store.setResultState(graphPath, { ...session.resultState, revision: "1" });
  const invalid = {
    ...next,
    resultState: { ...next.resultState, semanticInputHash: "f".repeat(64) },
  };
  expect(() => store.hydrate(graphPath, invalid)).toThrow("same semantic identity");
  unsubscribe();
  expect(notifications).toBe(0);
  expect(useGraphProjectionStore.getState()).toBe(current);
});
