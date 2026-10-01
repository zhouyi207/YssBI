import { afterEach, expect, it } from "vitest";
import { produce } from "immer";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { portAddressKey } from "@/features/domain/editorProjection";
import type { DiagnosticDto } from "@/shared/types/domain/editorProjection";
import { prepareGraphSessions, useGraphProjectionStore } from "./graphProjectionStore";

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

it("indexes the first blocking port diagnostic with ordered fallback and node-local ownership", () => {
  const { session, parts } = frame();
  const [first, second, third] = parts;
  const warning: DiagnosticDto = {
    code: "test.warning",
    messageKey: "test.warning",
    arguments: {},
    severity: "warning",
    blocking: false,
    location: { kind: "port", address: first.inputAddress },
    related: [],
  };
  const blocking: DiagnosticDto = {
    ...warning,
    code: "graph.input.unbound",
    severity: "error",
    blocking: true,
  };
  const outputWarning: DiagnosticDto = {
    ...warning,
    location: { kind: "port", address: first.outputAddress },
  };
  const secondWarning: DiagnosticDto = {
    ...warning,
    location: { kind: "port", address: second.inputAddress },
  };
  const instanceAddress = {
    kind: "instance" as const,
    nodeId: first.inputAddress.nodeId,
    templateKey: "value",
    instanceId: "first",
  };
  const instanceWarning: DiagnosticDto = {
    ...warning,
    location: { kind: "port", address: instanceAddress },
  };
  session.projection.nodes[0].ports.push({
    ...session.projection.nodes[0].ports[0],
    address: instanceAddress,
  });
  session.projection.nodes[0].diagnostics = [
    { ...blocking, location: { kind: "node", nodeId: first.inputAddress.nodeId } },
    warning,
    { ...warning, arguments: { detail: "later warning" } },
    blocking,
    { ...blocking, arguments: { detail: "later blocking" } },
    outputWarning,
    { ...outputWarning, arguments: { detail: "later output warning" } },
    { ...blocking, location: { kind: "port", address: third.inputAddress } },
    instanceWarning,
  ];
  session.projection.nodes[1].diagnostics = [secondWarning];
  session.projection.diagnostics = session.projection.nodes.flatMap((node) => node.diagnostics);
  session.projection.hasBlockingDiagnostics = true;
  session.projection.outcome = { type: "analysisBlocked" };
  useGraphProjectionStore.getState().install(graphPath, session);
  const index = useGraphProjectionStore.getState().graphEntities[graphPath].primaryPortDiagnostics;

  expect(Object.keys(index)).toHaveLength(4);
  expect(index[first.inputKey]).toBe(blocking);
  expect(index[first.outputKey]).toBe(outputWarning);
  expect(index[second.inputKey]).toBe(secondWarning);
  expect(index[third.inputKey]).toBeUndefined();
  expect(index[portAddressKey(instanceAddress)]).toBe(instanceWarning);
});

it("publishes sparse port diagnostics atomically and drops stale entries without rebuilding unaffected nodes", () => {
  const { session, parts } = frame();
  const [first, second, third] = parts;
  const firstId = first.inputAddress.nodeId;
  const secondId = second.inputAddress.nodeId;
  const warning: DiagnosticDto = {
    code: "test.warning",
    messageKey: "test.warning",
    arguments: {},
    severity: "warning",
    blocking: false,
    location: { kind: "port", address: first.inputAddress },
    related: [],
  };
  const outputWarning: DiagnosticDto = {
    ...warning,
    location: { kind: "port", address: first.outputAddress },
  };
  const otherWarning: DiagnosticDto = {
    ...warning,
    location: { kind: "port", address: second.inputAddress },
  };
  session.projection.nodes[0].diagnostics = [warning, outputWarning];
  session.projection.nodes[1].diagnostics = [otherWarning];
  session.projection.diagnostics = [warning, outputWarning, otherWarning];
  const store = useGraphProjectionStore.getState();
  store.install(graphPath, session);
  const before = useGraphProjectionStore.getState().graphEntities[graphPath];
  expect(before.primaryPortDiagnostics[first.inputKey]).toBe(before.nodes[firstId].diagnostics[0]);
  expect(before.primaryPortDiagnostics[third.inputKey]).toBeUndefined();
  expect(Object.isFrozen(before.primaryPortDiagnostics)).toBe(true);

  let publications = 0;
  const unsubscribe = useGraphProjectionStore.subscribe((state) => {
    publications++;
    const bucket = state.graphEntities[graphPath];
    for (const diagnostic of Object.values(bucket.primaryPortDiagnostics)) {
      if (diagnostic.location.kind === "port")
        expect(bucket.nodes[diagnostic.location.address.nodeId].diagnostics).toContain(diagnostic);
    }
  });
  try {
    const moved = produce(session, (draft) => {
      draft.editing.version.revision = "1";
      draft.projection.nodes[0].position.x += 1;
    });
    store.hydrate(graphPath, moved);
    expect(useGraphProjectionStore.getState().graphEntities[graphPath].primaryPortDiagnostics).toBe(
      before.primaryPortDiagnostics,
    );
    const nodeOnly = produce(moved, (draft) => {
      draft.editing.version.revision = "2";
      const diagnostic: DiagnosticDto = { ...warning, location: { kind: "node", nodeId: firstId } };
      draft.projection.nodes[0].diagnostics.push(diagnostic);
      draft.projection.diagnostics.push(diagnostic);
    });
    store.hydrate(graphPath, nodeOnly);
    expect(useGraphProjectionStore.getState().graphEntities[graphPath].primaryPortDiagnostics).toBe(
      before.primaryPortDiagnostics,
    );
    const changed = produce(nodeOnly, (draft) => {
      draft.editing.version.revision = "3";
      const diagnostic = { ...outputWarning, arguments: { detail: "changed" } };
      draft.projection.nodes[0].diagnostics = [diagnostic];
      draft.projection.diagnostics = [diagnostic, otherWarning];
    });
    store.hydrate(graphPath, changed);
    const updated = useGraphProjectionStore.getState().graphEntities[graphPath];
    expect(Object.keys(updated.primaryPortDiagnostics)).toHaveLength(2);
    expect(updated.primaryPortDiagnostics[first.inputKey]).toBeUndefined();
    expect(updated.primaryPortDiagnostics[first.outputKey]).toBe(
      updated.nodes[firstId].diagnostics[0],
    );
    expect(updated.primaryPortDiagnostics[second.inputKey]).toBe(
      before.primaryPortDiagnostics[second.inputKey],
    );
    expect(before.primaryPortDiagnostics[first.inputKey]).toBe(warning);

    const cleared = produce(changed, (draft) => {
      draft.editing.version.revision = "4";
      draft.projection.nodes[0].diagnostics = [];
      draft.projection.diagnostics = [otherWarning];
    });
    store.hydrate(graphPath, cleared);
    expect(
      useGraphProjectionStore.getState().graphEntities[graphPath].primaryPortDiagnostics[
        first.outputKey
      ],
    ).toBeUndefined();
    const removed = produce(cleared, (draft) => {
      draft.editing.version.revision = "5";
      draft.projection.nodes.splice(1, 1);
      draft.projection.diagnostics = [];
      draft.resultState.outputs = draft.resultState.outputs.filter(
        (entry) => entry.output.port.nodeId !== secondId,
      );
    });
    store.hydrate(graphPath, removed);
    expect(
      useGraphProjectionStore.getState().graphEntities[graphPath].primaryPortDiagnostics,
    ).toEqual({});
    expect(publications).toBe(5);
  } finally {
    unsubscribe();
  }
});

it("prepares retained and replacement graphs together while preserving unchanged and newer entries", () => {
  const stablePath = "events/Stable.yssbi-event";
  const removedPath = "events/Removed.yssbi-event";
  const addedPath = "events/Added.yssbi-event";
  const { session } = frame();
  const stable = makeGraphEditorSession(
    makeEditorProjectionFixture({ graphPath: stablePath }).projection,
  );
  stable.editing.version.revision = "5";
  const added = makeGraphEditorSession(
    makeEditorProjectionFixture({ graphPath: addedPath }).projection,
  );
  const store = useGraphProjectionStore.getState();
  store.install(graphPath, session);
  store.install(stablePath, stable);
  store.install(
    removedPath,
    makeGraphEditorSession(makeEditorProjectionFixture({ graphPath: removedPath }).projection),
  );
  const base = useGraphProjectionStore.getState();
  const moved = produce(session, (draft) => {
    draft.editing.version.revision = "1";
    draft.projection.nodes[0].position.x += 1;
  });
  const stale = produce(stable, (draft) => {
    draft.editing.version.revision = "4";
  });
  const replacements = [
    { graphPath, session: moved },
    { graphPath: stablePath, session: stale },
    { graphPath: addedPath, session: added },
  ];
  const retained = new Set([graphPath, stablePath, addedPath]);
  let notifications = 0;
  const unsubscribe = useGraphProjectionStore.subscribe(() => notifications++);
  const plan = prepareGraphSessions(replacements, retained, base);
  expect(useGraphProjectionStore.getState()).toBe(base);
  expect(notifications).toBe(0);
  expect(Object.keys(plan.state.sessions)).toEqual([graphPath, stablePath, addedPath]);
  expect(plan.state.sessions[stablePath]).toBe(base.sessions[stablePath]);
  expect(plan.state.graphEntities[stablePath]).toBe(base.graphEntities[stablePath]);
  expect(plan.state.resultStates[stablePath]).toBe(base.resultStates[stablePath]);
  expect(plan.state.graphEntities[removedPath]).toBeUndefined();
  expect(plan.state.resultStates[removedPath]).toBeUndefined();
  expect(plan.state.sessions[graphPath].projection).toBe(moved.projection);
  for (const table of [plan.state.sessions, plan.state.graphEntities, plan.state.resultStates])
    expect(Object.isFrozen(table)).toBe(true);
  useGraphProjectionStore.setState(plan.state);
  expect(notifications).toBe(1);
  const current = useGraphProjectionStore.getState();
  expect(prepareGraphSessions(replacements, retained, current).state).toBe(current);
  unsubscribe();
});

it("leaves all published graphs intact when a later frame or duplicate path rejects the batch", () => {
  const { session } = frame();
  useGraphProjectionStore.getState().install(graphPath, session);
  const base = useGraphProjectionStore.getState();
  const moved = produce(session, (draft) => {
    draft.editing.version.revision = "1";
    draft.projection.nodes[0].position.x += 1;
  });
  const otherPath = "events/Other.yssbi-event";
  const invalid = makeGraphEditorSession(
    makeEditorProjectionFixture({ graphPath: otherPath }).projection,
  );
  invalid.resultState = { ...invalid.resultState, semanticInputHash: "f".repeat(64) };
  let notifications = 0;
  const unsubscribe = useGraphProjectionStore.subscribe(() => notifications++);
  expect(() =>
    prepareGraphSessions(
      [
        { graphPath, session: moved },
        { graphPath: otherPath, session: invalid },
      ],
      undefined,
      base,
    ),
  ).toThrow("same semantic identity");
  expect(() =>
    prepareGraphSessions(
      [
        { graphPath, session: moved },
        { graphPath, session: moved },
      ],
      undefined,
      base,
    ),
  ).toThrow("Duplicate graph session");
  expect(useGraphProjectionStore.getState()).toBe(base);
  expect(base.sessions[graphPath].projection).toBe(session.projection);
  expect(base.sessions[otherPath]).toBeUndefined();
  expect(notifications).toBe(0);
  unsubscribe();
});

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
  const dangling = produce(next, (draft) => {
    draft.projection.connections.push({
      connectionId: "dangling",
      output: draft.projection.nodes[0].ports[0].address,
      input: { kind: "declared", nodeId: "missing", portKey: "input" },
      order: null,
    });
  });
  expect(() => store.hydrate(graphPath, dangling)).toThrow("references a missing port");
  unsubscribe();
  expect(notifications).toBe(0);
  expect(useGraphProjectionStore.getState()).toBe(current);
});

it("retains topology tables and unrelated entities when a structurally shared node moves", () => {
  const { session, parts } = frame();
  const store = useGraphProjectionStore.getState();
  store.install(graphPath, session);
  const before = useGraphProjectionStore.getState().graphEntities[graphPath];
  const next = produce(session, (draft) => {
    draft.editing.version.revision = "1";
    draft.projection.nodes[0].position.x += 100;
  });
  store.hydrate(graphPath, next);
  const after = useGraphProjectionStore.getState().graphEntities[graphPath];
  expect(useGraphProjectionStore.getState().sessions[graphPath].projection).toBe(next.projection);
  const movedId = parts[0].outputAddress.nodeId;
  expect(after.nodes[movedId].position.x).toBe(before.nodes[movedId].position.x + 100);
  expect(after.nodes[movedId].pinIds).toBe(before.nodes[movedId].pinIds);
  expect(after.nodes[movedId].parameterGroups).toBe(before.nodes[movedId].parameterGroups);
  expect(after.nodes[parts[1].outputAddress.nodeId]).toBe(
    before.nodes[parts[1].outputAddress.nodeId],
  );
  expect(after.graphNodes).toBe(before.graphNodes);
  expect(after.pins).toBe(before.pins);
  expect(after.connections).toBe(before.connections);
  expect(after.pinConnections).toBe(before.pinConnections);
  expect(after.blockedConnectionIds).toBe(before.blockedConnectionIds);
  expect(Object.isFrozen(after.nodes[movedId].position)).toBe(true);
});

it("keeps adjacency order and blocking diagnostics current across topology replacements", () => {
  const { session, parts } = frame();
  const [source, left, right] = parts;
  const out = portAddressKey(source.outputAddress);
  const leftIn = portAddressKey(left.inputAddress);
  const rightIn = portAddressKey(right.inputAddress);
  session.projection.connections = [
    { connectionId: "first", output: source.outputAddress, input: left.inputAddress, order: "a" },
    { connectionId: "second", output: source.outputAddress, input: right.inputAddress, order: "b" },
  ];
  const store = useGraphProjectionStore.getState();
  store.install(graphPath, session);
  const before = useGraphProjectionStore.getState().graphEntities[graphPath];
  const reordered = produce(session, (draft) => {
    draft.editing.version.revision = "1";
    draft.projection.nodes.reverse();
    draft.projection.connections.reverse();
    draft.projection.diagnostics.push({
      code: "test.blocked",
      messageKey: "test.blocked",
      arguments: {},
      severity: "error",
      blocking: true,
      location: { kind: "connection", connectionId: "second" },
      related: [],
    });
    draft.projection.hasBlockingDiagnostics = true;
    draft.projection.outcome = { type: "analysisBlocked" };
  });
  store.hydrate(graphPath, reordered);
  const order = useGraphProjectionStore.getState().graphEntities[graphPath];
  expect(order.graphNodes).toEqual([...before.graphNodes].reverse());
  expect(order.nodes).toBe(before.nodes);
  expect(order.pins).toBe(before.pins);
  expect(order.connections).toBe(before.connections);
  expect(order.pinConnections[out]).toEqual(["second", "first"]);
  expect(order.pinConnections[leftIn]).toBe(before.pinConnections[leftIn]);
  expect(order.blockedConnectionIds).toEqual({ second: true });

  const rerouted = produce(reordered, (draft) => {
    draft.editing.version.revision = "2";
    draft.projection.connections[0].input = left.inputAddress;
    draft.projection.diagnostics[0].blocking = false;
    draft.projection.hasBlockingDiagnostics = false;
    draft.projection.outcome = { type: "success" };
  });
  store.hydrate(graphPath, rerouted);
  const route = useGraphProjectionStore.getState().graphEntities[graphPath];
  expect(route.pinConnections[out]).toBe(order.pinConnections[out]);
  expect(route.pinConnections[leftIn]).toEqual(["second", "first"]);
  expect(route.pinConnections[rightIn]).toEqual([]);
  expect(route.connections.first).toBe(order.connections.first);
  expect(route.connections.second.to).toBe(leftIn);
  expect(route.blockedConnectionIds).toEqual({});

  const added = makeEditorProjectionFixture({
    graphPath,
    nodeId: "00000000-0000-0000-0000-000000000004",
  });
  const replaced = produce(rerouted, (draft) => {
    draft.editing.version.revision = "3";
    draft.projection.nodes.splice(0, 2, added.projection.nodes[0]);
    draft.projection.nodes[1].ports = draft.projection.nodes[1].ports.filter(
      (port) => port.direction === "output",
    );
    draft.projection.connections = [
      {
        connectionId: "third",
        output: source.outputAddress,
        input: added.inputAddress,
        order: null,
      },
    ];
    draft.projection.diagnostics = [];
  });
  store.hydrate(graphPath, replaced);
  const after = useGraphProjectionStore.getState().graphEntities[graphPath];
  expect(after.graphNodes).toEqual([added.inputAddress.nodeId, source.inputAddress.nodeId]);
  expect(Object.keys(after.nodes)).toHaveLength(2);
  expect(after.nodes[left.inputAddress.nodeId]).toBeUndefined();
  expect(after.nodes[right.inputAddress.nodeId]).toBeUndefined();
  expect(after.pins[leftIn]).toBeUndefined();
  expect(after.pins[rightIn]).toBeUndefined();
  expect(after.pins[portAddressKey(source.inputAddress)]).toBeUndefined();
  expect(after.pinConnections[leftIn]).toBeUndefined();
  expect(after.pinConnections[rightIn]).toBeUndefined();
  expect(after.pinConnections[out]).toEqual(["third"]);
  expect(after.pinConnections[portAddressKey(added.inputAddress)]).toEqual(["third"]);
  expect(Object.keys(after.connections)).toEqual(["third"]);
  expect(after.blockedConnectionIds).toBe(route.blockedConnectionIds);
});
