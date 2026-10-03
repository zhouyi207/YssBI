import { afterEach, expect, it } from "vitest";
import { produce } from "immer";
import {
  makeEditorProjectionFixture,
  makeGraphConstantFixture,
  makeGraphEditorSession,
  makeGraphProjectionJsonFixture,
  changeGraphProjectionJsonFixture,
} from "@/tests/helpers/editorProjectionFixtures";
import { portAddressKey } from "@/features/domain/editorProjection";
import type {
  DiagnosticDto,
  ParameterEditorDto,
  ParameterEditorSpecDto,
} from "@/shared/types/domain/editorProjection";
import type { ValueType } from "@/shared/types/domain/valueType";
import { prepareGraphSessions } from "@/features/core/dataStore/graphProjection";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { shareGraphConstants } from "@/features/core/dataStore/graphConstantProjection";
import { freezePublishedValue } from "@/shared/types/deepReadonly";
import { parseGraphDocumentDto } from "@/shared/types/dto/editorMutationWireParser";
import type { GraphConstantDto } from "@/shared/types/domain/editorMutation";
import { parseEditorGraphProjectionDto } from "@/shared/types/domain/editorProjectionParser";
import {
  makeDynamicKeyConstantFixture,
  changeDynamicKeyConstantFixture,
  shareGraphConstantsWithImmerReference,
} from "@/tests/benchmarks/graphConstantSharingReference";

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

afterEach(() => useResourceStore.getState().clear());

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
  useResourceStore.getState().installGraphSession(graphPath, session, { mode: "load" });
  const index = useResourceStore.getState().graphEntities[graphPath].primaryPortDiagnostics;

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
  const store = useResourceStore.getState();
  store.installGraphSession(graphPath, session, { mode: "load" });
  const before = useResourceStore.getState().graphEntities[graphPath];
  expect(before.primaryPortDiagnostics[first.inputKey]).toBe(before.nodes[firstId].diagnostics[0]);
  expect(before.primaryPortDiagnostics[third.inputKey]).toBeUndefined();
  expect(Object.isFrozen(before.primaryPortDiagnostics)).toBe(true);

  let publications = 0;
  const unsubscribe = useResourceStore.subscribe((state) => {
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
    store.installGraphSession(graphPath, moved);
    expect(useResourceStore.getState().graphEntities[graphPath].primaryPortDiagnostics).toBe(
      before.primaryPortDiagnostics,
    );
    const nodeOnly = produce(moved, (draft) => {
      draft.editing.version.revision = "2";
      const diagnostic: DiagnosticDto = { ...warning, location: { kind: "node", nodeId: firstId } };
      draft.projection.nodes[0].diagnostics.push(diagnostic);
      draft.projection.diagnostics.push(diagnostic);
    });
    store.installGraphSession(graphPath, nodeOnly);
    expect(useResourceStore.getState().graphEntities[graphPath].primaryPortDiagnostics).toBe(
      before.primaryPortDiagnostics,
    );
    const changed = produce(nodeOnly, (draft) => {
      draft.editing.version.revision = "3";
      const diagnostic = { ...outputWarning, arguments: { detail: "changed" } };
      draft.projection.nodes[0].diagnostics = [diagnostic];
      draft.projection.diagnostics = [diagnostic, otherWarning];
    });
    store.installGraphSession(graphPath, changed);
    const updated = useResourceStore.getState().graphEntities[graphPath];
    expect(Object.keys(updated.primaryPortDiagnostics)).toHaveLength(2);
    expect(updated.primaryPortDiagnostics[first.inputKey]).toBeUndefined();
    expect(updated.primaryPortDiagnostics[first.outputKey]).toBe(
      updated.nodes[firstId].diagnostics[0],
    );
    expect(updated.nodes[firstId].diagnostics).toBe(
      useResourceStore.getState().sessions[graphPath].projection.nodes[0].diagnostics,
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
    store.installGraphSession(graphPath, cleared);
    expect(
      useResourceStore.getState().graphEntities[graphPath].primaryPortDiagnostics[first.outputKey],
    ).toBeUndefined();
    const removed = produce(cleared, (draft) => {
      draft.editing.version.revision = "5";
      draft.projection.nodes.splice(1, 1);
      draft.projection.diagnostics = [];
      draft.resultState.outputs = draft.resultState.outputs.filter(
        (entry) => entry.output.port.nodeId !== secondId,
      );
    });
    store.installGraphSession(graphPath, removed);
    expect(useResourceStore.getState().graphEntities[graphPath].primaryPortDiagnostics).toEqual({});
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
  const store = useResourceStore.getState();
  store.installGraphSession(graphPath, session, { mode: "load" });
  store.installGraphSession(stablePath, stable, { mode: "load" });
  store.installGraphSession(
    removedPath,
    makeGraphEditorSession(makeEditorProjectionFixture({ graphPath: removedPath }).projection),
    { mode: "load" },
  );
  const base = useResourceStore.getState();
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
  const unsubscribe = useResourceStore.subscribe(() => notifications++);
  const plan = prepareGraphSessions(replacements, retained, base);
  expect(useResourceStore.getState()).toBe(base);
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
  useResourceStore.setState(plan.state);
  expect(notifications).toBe(1);
  const current = useResourceStore.getState();
  expect(prepareGraphSessions(replacements, retained, current).state).toBe(current);
  unsubscribe();
});

it("leaves all published graphs intact when a later frame or duplicate path rejects the batch", () => {
  const { session } = frame();
  useResourceStore.getState().installGraphSession(graphPath, session, { mode: "load" });
  const base = useResourceStore.getState();
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
  const unsubscribe = useResourceStore.subscribe(() => notifications++);
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
  expect(useResourceStore.getState()).toBe(base);
  expect(base.sessions[graphPath].projection).toBe(session.projection);
  expect(base.sessions[otherPath]).toBeUndefined();
  expect(notifications).toBe(0);
  unsubscribe();
});

it("publishes connection edits with matching results once and retains unrelated snapshot entities", () => {
  const { session, parts } = frame();
  session.projection.connections = [];
  for (const node of session.projection.nodes)
    for (const port of node.ports) {
      port.connections.current = 0;
      port.connections.canMove = false;
    }
  useResourceStore.getState().installGraphSession(graphPath, session, { mode: "load" });
  const before = useResourceStore.getState();
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
  const unsubscribe = useResourceStore.subscribe((state) => {
    const hash = state.sessions[graphPath].semanticInputHash;
    expect(state.graphEntities[graphPath].basis.semanticInputHash).toBe(hash);
    expect(state.resultStates[graphPath].semanticInputHash).toBe(hash);
    updates.push(hash);
  });
  useResourceStore.getState().installGraphSession(graphPath, next);
  unsubscribe();
  expect(updates).toEqual([next.projection.basis.semanticInputHash]);
  const after = useResourceStore.getState();
  const unaffected = parts[2].outputAddress.nodeId;
  expect(after.graphEntities[graphPath].nodes[unaffected]).toBe(
    before.graphEntities[graphPath].nodes[unaffected],
  );
  expect(after.graphEntities[graphPath].pins[portAddressKey(parts[2].outputAddress)]).toBe(
    before.graphEntities[graphPath].pins[portAddressKey(parts[2].outputAddress)],
  );
  expect(after.resultStates[graphPath].outputs[2]).toBe(before.resultStates[graphPath].outputs[2]);
  const projected = after.sessions[graphPath].projection.nodes[0];
  const node = after.graphEntities[graphPath].nodes[projected.nodeId];
  for (const field of [
    "position",
    "display",
    "capabilities",
    "portInstanceAdditions",
    "diagnostics",
  ] as const) {
    expect(node[field]).toBe(projected[field]);
    expect(node[field]).toBe(before.graphEntities[graphPath].nodes[projected.nodeId][field]);
  }
  const port = projected.ports.find((entry) => entry.direction === "output")!;
  const pin = after.graphEntities[graphPath].pins[parts[0].outputKey];
  expect(pin.connections).toBe(port.connections);
  expect(pin.connections).not.toBe(
    before.graphEntities[graphPath].pins[parts[0].outputKey].connections,
  );
  for (const field of ["address", "display", "input", "typeState", "resolvedSchema"] as const) {
    expect(pin[field]).toBe(port[field]);
    expect(pin[field]).toBe(before.graphEntities[graphPath].pins[parts[0].outputKey][field]);
  }
});

it("keeps newer execution results and rejects stale or inconsistent graph frames without publication", () => {
  const { session } = frame();
  const constantId = "00000000-0000-0000-0000-000000000004";
  session.document.constants = {
    [constantId]: {
      id: constantId,
      name: "Before",
      dataType: { kind: "Scalar", inner: "Numeric" },
      dataValue: { Integer: "1" },
    },
  };
  const store = useResourceStore.getState();
  store.installGraphSession(graphPath, session, { mode: "load" });
  const constants = useResourceStore.getState().sessions[graphPath].constants;
  store.setGraphResultState(graphPath, { ...session.resultState, revision: "3" });
  const results = useResourceStore.getState().resultStates[graphPath];
  const next = structuredClone(session);
  next.editing.version.revision = "2";
  next.resultState = { ...next.resultState, revision: "2" };
  store.installGraphSession(graphPath, next);
  expect(useResourceStore.getState().resultStates[graphPath]).toBe(results);
  expect(useResourceStore.getState().sessions[graphPath].constants).toBe(constants);
  const sessionBeforeResults = useResourceStore.getState().sessions[graphPath];
  store.installGraphSession(graphPath, {
    ...next,
    resultState: { ...next.resultState, revision: "4" },
  });
  expect(useResourceStore.getState().sessions[graphPath]).toBe(sessionBeforeResults);
  const beforeConstantChange = useResourceStore.getState();
  const changedConstant = produce(next, (draft) => {
    draft.document.constants![constantId].name = "After";
  });
  // Content is checked even when the incoming editing version is unchanged.
  store.installGraphSession(graphPath, changedConstant);
  const changed = useResourceStore.getState();
  expect(changed.sessions[graphPath].constants![constantId].name).toBe("After");
  expect(changed.sessions[graphPath].constants![constantId].dataValue).toBe(
    constants![constantId].dataValue,
  );
  expect(changed.graphEntities).toBe(beforeConstantChange.graphEntities);
  expect(constants![constantId].name).toBe("Before");
  store.installGraphSession(graphPath, structuredClone(changedConstant));
  expect(useResourceStore.getState()).toBe(changed);
  const current = useResourceStore.getState();
  let notifications = 0;
  const unsubscribe = useResourceStore.subscribe(() => notifications++);
  store.installGraphSession(graphPath, session);
  store.setGraphResultState(graphPath, { ...session.resultState, revision: "1" });
  const invalid = {
    ...next,
    resultState: { ...next.resultState, semanticInputHash: "f".repeat(64) },
  };
  expect(() => store.installGraphSession(graphPath, invalid)).toThrow("same semantic identity");
  const dangling = produce(next, (draft) => {
    draft.projection.connections.push({
      connectionId: "dangling",
      output: draft.projection.nodes[0].ports[0].address,
      input: { kind: "declared", nodeId: "missing", portKey: "input" },
      order: null,
    });
  });
  expect(() => store.installGraphSession(graphPath, dangling)).toThrow("references a missing port");
  const duplicateNode = produce(next, (draft) => {
    draft.projection.nodes.push(draft.projection.nodes[0]);
  });
  expect(() => store.installGraphSession(graphPath, duplicateNode)).toThrow("duplicate node");
  const duplicatePort = produce(next, (draft) => {
    draft.projection.nodes[0].ports.push(draft.projection.nodes[0].ports[0]);
  });
  expect(() => store.installGraphSession(graphPath, duplicatePort)).toThrow("duplicate port");
  unsubscribe();
  expect(notifications).toBe(0);
  expect(useResourceStore.getState()).toBe(current);
});

it("shares typed metadata while preserving dictionary keys, changed unions and incoming delta identity", () => {
  const { session, parts } = frame();
  const [first, second, third] = parts;
  session.projection.basis.resourceVersions = Object.fromEntries([
    ["__proto__", "v1"],
    ["other", "v1"],
  ]);
  session.projection.basis.resourceObservations = Object.fromEntries([
    ["__proto__", { kind: "present", version: "v1" }],
    ["other", { kind: "present", version: "v1" }],
  ]);
  session.projection.connections = [second, third].map((part, index) => ({
    connectionId: String(index),
    output: first.outputAddress,
    input: part.inputAddress,
    order: "a",
  }));
  const node = session.projection.nodes[0];
  node.portInstanceAdditions = ["first", "second"].map((templateKey) => ({
    templateKey,
    label: templateKey,
    direction: "input",
    canAdd: true,
  }));
  node.ports[0].typeState = {
    status: "exact",
    display: "Array",
    dataType: { kind: "Array", inner: { kind: "Scalar", inner: "Numeric" } },
  };
  node.ports[0].resolvedSchema = {
    kind: "fixed",
    fields: [
      { name: "a", scalarType: "Numeric" },
      { name: "b", scalarType: "Text" },
    ],
  };
  const diagnostic: DiagnosticDto = {
    code: "test.metadata",
    messageKey: "test.metadata",
    severity: "warning",
    blocking: false,
    arguments: Object.fromEntries([["__proto__", "reserved"]]),
    location: { kind: "port", address: first.inputAddress },
    related: [{ kind: "graph" }, { kind: "port", address: first.outputAddress }],
  };
  node.diagnostics = [diagnostic];
  session.projection.diagnostics = [diagnostic];
  parseEditorGraphProjectionDto(session.projection);
  const store = useResourceStore.getState();
  store.installGraphSession(graphPath, session, { mode: "load" });
  const before = useResourceStore.getState().sessions[graphPath].projection;
  const fresh = structuredClone(session);
  fresh.projection.basis.resourceVersions.other = "v2";
  fresh.projection.basis.resourceObservations.other.version = "v2";
  fresh.projection.connections[0].order = "b";
  const freshNode = fresh.projection.nodes[0];
  freshNode.portInstanceAdditions[0].canAdd = false;
  if (freshNode.ports[0].typeState.status === "exact")
    freshNode.ports[0].typeState.display = "Changed";
  freshNode.ports[0].resolvedSchema!.kind = "derived";
  freshNode.diagnostics[0].messageKey = "test.changed";
  freshNode.diagnostics[0].arguments.detail = "changed";
  freshNode.diagnostics[0].related[1] = { kind: "port", address: second.inputAddress };
  parseEditorGraphProjectionDto(fresh.projection);
  store.installGraphSession(graphPath, fresh);
  const shared = useResourceStore.getState().sessions[graphPath].projection;
  const sharedNode = shared.nodes[0];
  expect(shared).toEqual(fresh.projection);
  expect(shared.basis.resourceObservations.__proto__).toBe(
    before.basis.resourceObservations.__proto__,
  );
  expect(Object.prototype.hasOwnProperty.call(shared.basis.resourceObservations, "__proto__")).toBe(
    true,
  );
  expect(Object.getPrototypeOf(shared.basis.resourceObservations)).toBe(Object.prototype);
  expect(shared.connections[0].output).toBe(before.connections[0].output);
  expect(shared.connections[0].input).toBe(before.connections[0].input);
  expect(shared.connections[1]).toBe(before.connections[1]);
  expect(sharedNode.portInstanceAdditions[1]).toBe(node.portInstanceAdditions[1]);
  expect(sharedNode.ports[0].resolvedSchema!.fields).toBe(node.ports[0].resolvedSchema!.fields);
  expect(sharedNode.ports[0].typeState).toMatchObject({
    dataType: node.ports[0].typeState.dataType,
  });
  if (sharedNode.ports[0].typeState.status === "exact")
    expect(sharedNode.ports[0].typeState.dataType).toBe(node.ports[0].typeState.dataType);
  expect(sharedNode.diagnostics[0].location).toBe(diagnostic.location);
  expect(sharedNode.diagnostics[0].related[0]).toBe(diagnostic.related[0]);
  expect(sharedNode.diagnostics[0].related[1]).toBe(freshNode.diagnostics[0].related[1]);
  expect(
    Object.prototype.hasOwnProperty.call(sharedNode.diagnostics[0].arguments, "__proto__"),
  ).toBe(true);
  expect(sharedNode.diagnostics[0].arguments.__proto__).toBe("reserved");

  const delta = produce({ ...fresh, projection: shared }, (draft) => {
    draft.editing.version.revision = "1";
    draft.projection.basis.resourceObservations.other = { kind: "absent", version: null };
    draft.projection.connections.shift();
    const changingNode = draft.projection.nodes[0];
    changingNode.portInstanceAdditions.pop();
    changingNode.ports[0].resolvedSchema!.fields.reverse();
    changingNode.ports[0].typeState = {
      status: "constrained",
      display: "Numeric or Text",
      domain: [
        { kind: "Scalar", inner: "Numeric" },
        { kind: "Scalar", inner: "Text" },
      ],
    };
    changingNode.diagnostics[0].related.pop();
  });
  store.installGraphSession(graphPath, delta);
  expect(useResourceStore.getState().sessions[graphPath].projection).toBe(delta.projection);
  expect(delta.projection.nodes[0].ports[0].typeState).not.toHaveProperty("dataType");
  expect(node.ports[0].resolvedSchema!.fields.map((field) => field.name)).toEqual(["a", "b"]);
  expect(before.connections).toHaveLength(2);
  expect(before.basis.resourceObservations.other.version).toBe("v1");

  const last = structuredClone(delta);
  delete last.projection.basis.resourceObservations.other;
  Object.assign(last.projection.basis.resourceObservations, {
    constructor: { kind: "present", version: "v3" },
  });
  last.projection.nodes[0].ports[0].resolvedSchema = null;
  if (last.projection.nodes[0].ports[0].typeState.status === "constrained")
    last.projection.nodes[0].ports[0].typeState.display = "Renamed";
  store.installGraphSession(graphPath, last);
  const final = useResourceStore.getState().sessions[graphPath].projection;
  expect(final.basis.resourceObservations).toEqual(last.projection.basis.resourceObservations);
  expect(final.basis.resourceObservations.__proto__).toBe(
    before.basis.resourceObservations.__proto__,
  );
  expect(final.basis.resourceObservations.constructor).toBe(
    last.projection.basis.resourceObservations.constructor,
  );
  expect(Object.prototype.hasOwnProperty.call(final.basis.resourceObservations, "other")).toBe(
    false,
  );
  expect(delta.projection.basis.resourceObservations.other).toEqual({
    kind: "absent",
    version: null,
  });
  expect(final.nodes[0].ports[0].resolvedSchema).toBeNull();
  if (
    final.nodes[0].ports[0].typeState.status === "constrained" &&
    delta.projection.nodes[0].ports[0].typeState.status === "constrained"
  )
    expect(final.nodes[0].ports[0].typeState.domain).toBe(
      delta.projection.nodes[0].ports[0].typeState.domain,
    );
  store.installGraphSession(graphPath, structuredClone(last));
  expect(useResourceStore.getState().sessions[graphPath].projection).toBe(final);
});

it("shares relational editor fields and recursive types while preserving incoming delta identity", () => {
  const { session } = frame();
  const project: Extract<ParameterEditorSpecDto, { kind: "projectColumns" }> = {
    kind: "projectColumns",
    allowEmpty: true,
    available: true,
    unavailableReason: null,
    options: [
      { name: "age", dataType: "Numeric" },
      { name: "label", dataType: "Text" },
    ],
    value: ["age"],
  };
  const filter: Extract<ParameterEditorSpecDto, { kind: "filterPredicate" }> = {
    kind: "filterPredicate",
    available: true,
    unavailableReason: null,
    columns: [
      {
        name: "age",
        dataType: "Numeric",
        operators: ["equal", "isNull"],
        literalTypes: ["integer"],
      },
    ],
    value: { column: "age", operator: "equal", value: { type: "integer", value: "18" } },
  };
  const type: ValueType = {
    kind: "OneOf",
    inner: [
      { kind: "Array", inner: { kind: "Scalar", inner: "Numeric" } },
      { kind: "DataSeries", inner: { kind: "Struct", inner: "Row" } },
      { kind: "Object" },
      { kind: "Any" },
      { kind: "DataFrame" },
    ],
  };
  const parameter = (key: string, editor: ParameterEditorSpecDto): ParameterEditorDto => ({
    key,
    display: { title: key, description: null },
    editor,
    presentation: "detailPanel",
    valueType: structuredClone(type),
    multiline: false,
    value: null,
  });
  const node = session.projection.nodes[0];
  node.parameterGroups = [
    {
      key: "relational",
      display: { title: "Relational", description: null },
      parameters: [
        parameter("project", project),
        parameter("filter", filter),
        parameter("select", { kind: "select", options: ["a", "b"] }),
      ],
    },
  ];
  node.ports[0].typeState = { status: "exact", display: "Union", dataType: structuredClone(type) };
  node.ports[1].typeState = {
    status: "constrained",
    display: "Union",
    domain: [structuredClone(type)],
  };
  parseEditorGraphProjectionDto(session.projection);
  const store = useResourceStore.getState();
  store.installGraphSession(graphPath, session, { mode: "load" });
  const before = useResourceStore.getState().sessions[graphPath].projection;
  const fresh = structuredClone(session);
  const incoming = fresh.projection.nodes[0].parameterGroups[0].parameters;
  const projectNext = incoming[0].editor as typeof project;
  projectNext.available = false;
  projectNext.unavailableReason = "Changed";
  projectNext.options[0].dataType = "Text";
  const filterNext = incoming[1].editor as typeof filter;
  filterNext.columns[0].name = "renamed";
  filterNext.value!.column = "renamed";
  (incoming[2].editor as Extract<ParameterEditorSpecDto, { kind: "select" }>).options!.reverse();
  const parameterType = incoming[0].valueType as typeof type;
  const portType = fresh.projection.nodes[0].ports[0].typeState;
  const portDomain = fresh.projection.nodes[0].ports[1].typeState;
  if (portType.status !== "exact" || portDomain.status !== "constrained")
    throw new Error("fixture type state");
  for (const union of [parameterType, portType.dataType, portDomain.domain[0]]) {
    if (union?.kind !== "OneOf" || union.inner[0].kind !== "Array")
      throw new Error("fixture union");
    union.inner[0] = { kind: "DataSeries", inner: union.inner[0].inner };
  }
  parseEditorGraphProjectionDto(fresh.projection);
  store.installGraphSession(graphPath, fresh);
  const shared = useResourceStore.getState().sessions[graphPath].projection;
  const parameters = shared.nodes[0].parameterGroups[0].parameters;
  const projectShared = parameters[0].editor as typeof project;
  const filterShared = parameters[1].editor as typeof filter;
  expect(shared).toEqual(fresh.projection);
  expect(projectShared.options[1]).toBe(project.options[1]);
  expect(projectShared.value).toBe(project.value);
  expect(filterShared.columns[0].operators).toBe(filter.columns[0].operators);
  expect(filterShared.columns[0].literalTypes).toBe(filter.columns[0].literalTypes);
  expect(filterShared.value!.value).toBe(filter.value!.value);
  const previousTypes = [
    before.nodes[0].parameterGroups[0].parameters[0].valueType,
    node.ports[0].typeState.dataType,
    node.ports[1].typeState.domain[0],
  ];
  const sharedExact = shared.nodes[0].ports[0].typeState;
  const sharedDomain = shared.nodes[0].ports[1].typeState;
  if (sharedExact.status !== "exact" || sharedDomain.status !== "constrained")
    throw new Error("shared type state");
  for (const [index, union] of [
    parameters[0].valueType,
    sharedExact.dataType,
    sharedDomain.domain[0],
  ].entries()) {
    const previousType = previousTypes[index];
    if (
      union?.kind !== "OneOf" ||
      previousType?.kind !== "OneOf" ||
      union.inner[0].kind !== "DataSeries" ||
      previousType.inner[0].kind !== "Array"
    )
      throw new Error("shared union");
    expect(union.inner[0].inner).toBe(previousType.inner[0].inner);
    for (let item = 1; item < union.inner.length; item++)
      expect(union.inner[item]).toBe(previousType.inner[item]);
  }
  const delta = produce({ ...fresh, projection: shared }, (draft) => {
    const parameters = draft.projection.nodes[0].parameterGroups[0].parameters;
    parameters[0].editor = { kind: "auto" };
    parameters[0].valueType = null;
    const editor = parameters[1].editor;
    if (editor.kind === "filterPredicate") editor.value = { column: "renamed", operator: "isNull" };
    parameters[2].editor = { kind: "select", options: null };
    parameters.reverse();
    const domain = draft.projection.nodes[0].ports[1].typeState;
    if (domain.status === "constrained") domain.domain = [{ kind: "Scalar", inner: "Text" }];
  });
  parseEditorGraphProjectionDto(delta.projection);
  store.installGraphSession(graphPath, delta);
  expect(useResourceStore.getState().sessions[graphPath].projection).toBe(delta.projection);
  store.installGraphSession(graphPath, structuredClone(delta));
  expect(useResourceStore.getState().sessions[graphPath].projection).toBe(delta.projection);
  expect(project.available).toBe(true);
  expect(filter.value!.value).toEqual({ type: "integer", value: "18" });
  expect(type.inner[0].kind).toBe("Array");
});

it("retains constant branch identities and legal object keys across snapshots and shared deltas", () => {
  const firstId = "00000000-0000-0000-0000-000000000004";
  const secondId = "00000000-0000-0000-0000-000000000005";
  const previous: Record<string, ReturnType<typeof makeGraphConstantFixture>> = {
    [firstId]: makeGraphConstantFixture(firstId),
    [secondId]: makeGraphConstantFixture(secondId),
  };
  const validate = (constants: Record<string, GraphConstantDto>) => {
    freezePublishedValue(constants);
    parseGraphDocumentDto({
      constants,
      nodes: {},
      port_bindings: [],
      connections: {},
      input_states: [],
    });
  };
  validate(previous);
  const share = shareGraphConstants;
  expect(share(undefined, previous)).toBe(previous);
  expect(share(previous, previous)).toBe(previous);
  expect(share(previous, undefined)).toBeUndefined();
  const equal = structuredClone(previous);
  validate(equal);
  expect(share(previous, equal)).toBe(previous);

  const renamed = structuredClone(previous);
  renamed[firstId].name = "Renamed";
  validate(renamed);
  const nameOnly = share(previous, renamed)!;
  expect(nameOnly).toEqual(renamed);
  expect(nameOnly[firstId].dataValue).toBe(previous[firstId].dataValue);
  expect(nameOnly[firstId].tags).toBe(previous[firstId].tags);
  expect(nameOnly[secondId]).toBe(previous[secondId]);

  const next = structuredClone(previous);
  next[firstId].dataValue.Object.left.List[0].Integer = "999";
  validate(next);
  const changed = share(previous, next)!;
  const value = changed[firstId].dataValue as (typeof next)[string]["dataValue"];
  expect(changed).toEqual(next);
  expect(changed).not.toBe(previous);
  expect(value.Object.left.List[0]).toBe(next[firstId].dataValue.Object.left.List[0]);
  expect(value.Object.left.List[1]).toBe(previous[firstId].dataValue.Object.left.List[1]);
  expect(value.Object.right).toBe(previous[firstId].dataValue.Object.right);
  expect(value.Object.metadata).toBe(previous[firstId].dataValue.Object.metadata);
  expect(changed[secondId]).toBe(previous[secondId]);

  const delta = produce(previous, (draft) => {
    draft[firstId].dataValue.Object.left.List[0].Integer = "999";
  });
  expect(share(previous, delta)).toBe(delta);

  const removed = structuredClone(previous);
  delete removed[secondId];
  delete (removed[firstId] as GraphConstantDto).description;
  removed[firstId].dataValue.Object.left.List.splice(1);
  removed[firstId].tags.reverse();
  validate(removed);
  const smaller = share(previous, removed)!;
  const smallerValue = smaller[firstId].dataValue as (typeof next)[string]["dataValue"];
  expect(smaller).toEqual(removed);
  expect(smaller[secondId]).toBeUndefined();
  expect(Object.prototype.hasOwnProperty.call(smaller[firstId], "description")).toBe(false);
  expect(smallerValue.Object.left.List[0]).toBe(previous[firstId].dataValue.Object.left.List[0]);
  expect(smallerValue.Object.right).toBe(previous[firstId].dataValue.Object.right);
  expect(smaller[firstId].tags).toBe(removed[firstId].tags);
  const added = produce(previous, (draft) => {
    draft[firstId].dataValue.Object.left.List.push({ Integer: "16" });
    delete (draft[firstId] as GraphConstantDto).description;
  });
  expect(share(previous, added)).toBe(added);
  expect(share(previous, {})).toEqual({});
  expect(previous[firstId].name).toBe(`Constant ${firstId}`);
  expect(previous[firstId].dataValue.Object.left.List[0].Integer).toBe("0");
  expect(previous[firstId].dataValue.Object.left.List).toHaveLength(16);
  expect(previous[firstId].description).toBe("Nested constant fixture");
  expect(previous[firstId].tags).toEqual(["fixture", "nested"]);
  expect(Object.keys(previous)).toHaveLength(2);

  const typedPrevious = {
    [firstId]: {
      ...previous[firstId],
      dataType: {
        kind: "OneOf",
        inner: [
          { kind: "Array", inner: { kind: "Scalar", inner: "Numeric" } },
          { kind: "Struct", inner: "Row" },
        ],
      } as ValueType,
    },
  };
  validate(typedPrevious);
  const typedNext = structuredClone(typedPrevious);
  const incomingType = typedNext[firstId].dataType;
  const oldType = typedPrevious[firstId].dataType;
  if (
    incomingType.kind !== "OneOf" ||
    oldType.kind !== "OneOf" ||
    incomingType.inner[0].kind !== "Array" ||
    oldType.inner[0].kind !== "Array"
  )
    throw new Error("fixture constant type");
  incomingType.inner[0] = { kind: "DataSeries", inner: incomingType.inner[0].inner };
  validate(typedNext);
  const typedShared = share(typedPrevious, typedNext)!;
  const sharedType = typedShared[firstId].dataType;
  if (sharedType.kind !== "OneOf" || sharedType.inner[0].kind !== "DataSeries")
    throw new Error("shared constant type");
  expect(typedShared).toEqual(typedNext);
  expect(sharedType.inner[0].inner).toBe(oldType.inner[0].inner);
  expect(sharedType.inner[1]).toBe(oldType.inner[1]);
  const typedDelta = produce(typedShared, (draft) => {
    const type = draft[firstId].dataType;
    if (type.kind === "OneOf") type.inner.pop();
  });
  expect(share(typedShared, typedDelta)).toBe(typedDelta);
  expect(share(typedShared, structuredClone(typedShared))).toBe(typedShared);
  expect(oldType.inner[0].kind).toBe("Array");

  const specialPrevious = {
    [firstId]: {
      ...makeGraphConstantFixture(firstId),
      dataValue: {
        Object: Object.fromEntries([
          ["__proto__", { Integer: "1" }],
          ["other", { Integer: "2" }],
        ]),
      },
      tabular: {
        columns: Object.fromEntries([
          ["__proto__", [1]],
          ["other", [2]],
        ]),
      },
    },
  };
  validate(specialPrevious);
  const specialNext = structuredClone(specialPrevious);
  specialNext[firstId].dataValue.Object.other.Integer = "3";
  specialNext[firstId].tabular.columns.other[0] = 3;
  validate(specialNext);
  const { session } = frame();
  session.document.constants = specialPrevious;
  useResourceStore.getState().installGraphSession(graphPath, session, { mode: "load" });
  useResourceStore.getState().installGraphSession(graphPath, {
    ...session,
    document: { ...session.document, constants: specialNext },
  });
  const special = useResourceStore.getState().sessions[graphPath].constants!;
  const specialValue = special[firstId]
    .dataValue as (typeof specialNext)[typeof firstId]["dataValue"];
  expect(special).toEqual(specialNext);
  expect(Object.prototype.hasOwnProperty.call(specialValue.Object, "__proto__")).toBe(true);
  expect(Object.getPrototypeOf(specialValue.Object)).toBe(Object.prototype);
  expect(specialValue.Object.__proto__).toBe(specialPrevious[firstId].dataValue.Object.__proto__);
  expect(special[firstId].tabular!.columns.__proto__).toBe(
    specialPrevious[firstId].tabular.columns.__proto__,
  );
  expect(Object.getPrototypeOf(special[firstId].tabular!.columns)).toBe(Object.prototype);
  const tabularDelta = produce(special, (draft) => {
    draft[firstId].tabular!.columns.other[0] = 4;
  });
  validate(tabularDelta);
  expect(share(special, tabularDelta)).toBe(tabularDelta);
  const renamedColumns = structuredClone(tabularDelta);
  delete renamedColumns[firstId].tabular!.columns.other;
  Object.assign(renamedColumns[firstId].tabular!.columns, { constructor: [5] });
  validate(renamedColumns);
  const renamedTable = share(tabularDelta, renamedColumns)!;
  expect(renamedTable).toEqual(renamedColumns);
  expect(renamedTable[firstId].tabular!.columns.__proto__).toBe(
    tabularDelta[firstId].tabular!.columns.__proto__,
  );
  expect(renamedTable[firstId].tabular!.columns.constructor).toBe(
    renamedColumns[firstId].tabular!.columns.constructor,
  );
  expect(
    Object.prototype.hasOwnProperty.call(renamedTable[firstId].tabular!.columns, "other"),
  ).toBe(false);
  expect(share(renamedTable, structuredClone(renamedTable))).toBe(renamedTable);
  const withoutTable = produce(renamedTable, (draft) => {
    delete draft[firstId].tabular;
  });
  expect(share(renamedTable, withoutTable)).toBe(withoutTable);
  expect(specialPrevious[firstId].tabular.columns.other).toEqual([2]);
});

it("keeps the Immer benchmark reference equivalent for legal dynamic JSON keys and shared deltas", () => {
  const firstId = "00000000-0000-0000-0000-000000000001";
  const secondId = "00000000-0000-0000-0000-000000000002";
  const previous = {
    [firstId]: makeDynamicKeyConstantFixture(firstId),
    [secondId]: makeDynamicKeyConstantFixture(secondId),
  };
  const validate = (constants: Record<string, GraphConstantDto>) => {
    freezePublishedValue(constants);
    parseGraphDocumentDto({
      constants,
      nodes: {},
      port_bindings: [],
      connections: {},
      input_states: [],
    });
  };
  validate(previous);
  for (const share of [shareGraphConstants, shareGraphConstantsWithImmerReference]) {
    expect(share(previous, structuredClone(previous))).toBe(previous);
    const incoming = structuredClone(previous);
    changeDynamicKeyConstantFixture(incoming[firstId]);
    delete (incoming[firstId] as Partial<GraphConstantDto>).description;
    validate(incoming);
    const result = share(previous, incoming)!;
    expect(result).toEqual(incoming);
    expect(result[secondId]).toBe(previous[secondId]);
    const value = result[firstId].dataValue as (typeof incoming)[typeof firstId]["dataValue"];
    const before = previous[firstId].dataValue.Object;
    const after = incoming[firstId].dataValue.Object;
    expect(value.Object.left.List[0]).toBe(after.left.List[0]);
    expect(value.Object.left.List[1]).toBe(before.left.List[1]);
    expect(value.Object.right).toBe(before.right);
    expect(value.Object.metadata.Object.__proto__).toBe(after.metadata.Object.__proto__);
    expect(value.Object.metadata.Object.constructor.Object.keep).toBe(
      before.metadata.Object.constructor.Object.keep,
    );
    expect(value.Object.metadata.Object.constructor.Object.change).toBe(
      after.metadata.Object.constructor.Object.change,
    );
    expect(Object.prototype.hasOwnProperty.call(value.Object.metadata.Object, "__proto__")).toBe(
      true,
    );
    expect(Object.getPrototypeOf(value.Object.metadata.Object)).toBe(Object.prototype);
    expect(value.Object.metadata.Object).not.toHaveProperty("enabled");
    expect(result[firstId]).not.toHaveProperty("description");
    const delta = produce(previous, (draft) => changeDynamicKeyConstantFixture(draft[firstId]));
    validate(delta);
    expect(share(previous, delta)).toBe(delta);
    const removedKeys = structuredClone(previous);
    const removedMetadata = removedKeys[firstId].dataValue.Object.metadata.Object;
    Reflect.deleteProperty(removedMetadata, "__proto__");
    Reflect.deleteProperty(removedMetadata, "constructor");
    Reflect.deleteProperty(removedKeys, secondId);
    validate(removedKeys);
    const withoutKeys = share(previous, removedKeys)!;
    expect(withoutKeys).toEqual(removedKeys);
    expect(
      Object.prototype.hasOwnProperty.call(
        (withoutKeys[firstId].dataValue as (typeof incoming)[typeof firstId]["dataValue"]).Object
          .metadata.Object,
        "__proto__",
      ),
    ).toBe(false);
    const replacedKey = structuredClone(previous);
    Object.assign(replacedKey[firstId].dataValue.Object.metadata.Object, {
      ["__proto__"]: { String: "replacement" },
    });
    validate(replacedKey);
    const replaced = share(previous, replacedKey)!;
    expect(replaced).toEqual(replacedKey);
    const replacedMetadata = (
      replaced[firstId].dataValue as (typeof incoming)[typeof firstId]["dataValue"]
    ).Object.metadata.Object;
    expect(replacedMetadata.__proto__).toBe(
      replacedKey[firstId].dataValue.Object.metadata.Object.__proto__,
    );
    expect(replacedMetadata.constructor).toBe(before.metadata.Object.constructor);

    const table: Record<string, GraphConstantDto> = {
      [firstId]: {
        id: firstId,
        name: "Table",
        dataType: { kind: "DataFrame" },
        dataValue: "Null",
        tabular: {
          columns: { ["__proto__"]: [1, null], constructor: [true, false], other: ["a", "b"] },
        },
      },
    };
    validate(table);
    const nextTable = structuredClone(table);
    nextTable[firstId].tabular!.columns.other.reverse();
    Reflect.deleteProperty(nextTable[firstId].tabular!.columns, "constructor");
    validate(nextTable);
    const sharedTable = share(table, nextTable)!;
    expect(sharedTable).toEqual(nextTable);
    expect(sharedTable[firstId].tabular!.columns.__proto__).toBe(
      table[firstId].tabular!.columns.__proto__,
    );
    expect(sharedTable[firstId].tabular!.columns.other).toBe(
      nextTable[firstId].tabular!.columns.other,
    );
    expect(
      Object.prototype.hasOwnProperty.call(sharedTable[firstId].tabular!.columns, "constructor"),
    ).toBe(false);
    expect(previous[firstId].dataValue.Object.metadata.Object.__proto__.List[0].Integer).toBe("40");
  }
});

it("shares result records by output and connection identity across frame reorders and query updates", () => {
  const { session, parts } = frame();
  session.resultState = {
    ...session.resultState,
    connections: [parts[1], parts[2]].map((part) => ({
      output: { graphPath, port: parts[0].outputAddress },
      input: part.inputAddress,
      state: "valid",
    })),
  };
  const store = useResourceStore.getState();
  store.installGraphSession(graphPath, session, { mode: "load" });
  const before = useResourceStore.getState();
  const results = before.resultStates[graphPath];
  let notifications = 0;
  const unsubscribe = useResourceStore.subscribe(() => notifications++);
  try {
    store.setGraphResultState(graphPath, structuredClone(results));
    expect(notifications).toBe(0);
    const reordered = produce(session, (draft) => {
      draft.resultState.revision = "2";
      draft.resultState.outputs.reverse();
      draft.resultState.connections.reverse();
    });
    store.installGraphSession(graphPath, structuredClone(reordered));
    const installed = useResourceStore.getState();
    const order = installed.resultStates[graphPath];
    expect(installed.sessions[graphPath]).toBe(before.sessions[graphPath]);
    expect(installed.graphEntities).toBe(before.graphEntities);
    expect(order.outputs[0]).toBe(results.outputs[2]);
    expect(order.outputs[2]).toBe(results.outputs[0]);
    expect(order.connections[0]).toBe(results.connections[1]);
    expect(order.connections[1]).toBe(results.connections[0]);
    expect(notifications).toBe(1);

    const changed = produce(structuredClone(order), (draft) => {
      draft.revision = "3";
      draft.outputs[0].state = "stale";
      draft.outputs[0].resultId = null;
      draft.outputs.pop();
      draft.connections[0].state = "stale";
    });
    store.setGraphResultState(graphPath, changed);
    const after = useResourceStore.getState().resultStates[graphPath];
    expect(after).toEqual(changed);
    expect(after.outputs[0].output).toBe(order.outputs[0].output);
    expect(after.outputs[1]).toBe(order.outputs[1]);
    expect(after.connections[0].output).toBe(order.connections[0].output);
    expect(after.connections[0].input).toBe(order.connections[0].input);
    expect(after.connections[1]).toBe(order.connections[1]);
    expect(results.outputs).toHaveLength(3);
    expect(results.outputs[2].state).toBe("valid");
    expect(Object.isFrozen(after.outputs[0])).toBe(true);
    store.setGraphResultState(graphPath, structuredClone(after));
    expect(notifications).toBe(2);
  } finally {
    unsubscribe();
  }
});

it("retains topology tables and unrelated entities when a structurally shared node moves", () => {
  const { session, parts } = frame();
  const opaque = (key: string) =>
    Object.fromEntries([
      ["__proto__", { key: "reserved" }],
      ["nested", { key }],
    ]);
  const parameter = (key: string): ParameterEditorDto => ({
    key,
    display: { title: key, description: null },
    editor: { kind: "auto" },
    presentation: "detailPanel",
    valueType: { kind: "Any" },
    multiline: false,
    value: opaque(key),
  });
  session.projection.nodes[0].parameterGroups = [
    {
      key: "first",
      display: { title: "First", description: null },
      parameters: [parameter("a"), parameter("b")],
    },
    {
      key: "second",
      display: { title: "Second", description: null },
      parameters: [parameter("c")],
    },
  ];
  const inputIndex = session.projection.nodes[0].ports.findIndex((port) => port.input);
  session.projection.nodes[0].ports[inputIndex].input!.literalOverride = opaque("literal");
  session.projection.nodes[0].ports[inputIndex].input!.protocolDefault = opaque("default");
  const store = useResourceStore.getState();
  store.installGraphSession(graphPath, session, { mode: "load" });
  const before = useResourceStore.getState().graphEntities[graphPath];
  const next = produce(session, (draft) => {
    draft.editing.version.revision = "1";
    draft.projection.nodes[0].position.x += 100;
  });
  store.installGraphSession(graphPath, next);
  const after = useResourceStore.getState().graphEntities[graphPath];
  expect(useResourceStore.getState().sessions[graphPath].projection).toBe(next.projection);
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

  const reordered = produce(next, (draft) => {
    draft.editing.version.revision = "2";
    draft.projection.nodes.reverse();
    draft.projection.nodes[0].ports.reverse();
    draft.projection.nodes[2].parameterGroups.reverse();
    draft.projection.nodes[2].parameterGroups[1].parameters.reverse();
  });
  store.installGraphSession(graphPath, reordered);
  expect(useResourceStore.getState().sessions[graphPath].projection).toBe(reordered.projection);
  expect(useResourceStore.getState().graphEntities[graphPath].nodes[movedId].parameterGroups).toBe(
    reordered.projection.nodes[2].parameterGroups,
  );
  expect(reordered.projection.nodes[2].parameterGroups[0]).toBe(
    next.projection.nodes[0].parameterGroups[1],
  );
  expect(reordered.projection.nodes[2].parameterGroups[1].parameters[0]).toBe(
    next.projection.nodes[0].parameterGroups[0].parameters[1],
  );

  const fresh = structuredClone(reordered);
  fresh.editing.version.revision = "3";
  fresh.projection.nodes[2].parameterGroups.reverse();
  fresh.projection.nodes[2].parameterGroups[0].parameters.reverse();
  store.installGraphSession(graphPath, fresh);
  const canonical = useResourceStore.getState().sessions[graphPath].projection.nodes[2];
  expect(canonical.parameterGroups[0].parameters[0]).toBe(
    next.projection.nodes[0].parameterGroups[0].parameters[0],
  );
  expect(canonical.parameterGroups[1]).toBe(next.projection.nodes[0].parameterGroups[1]);
  expect(useResourceStore.getState().graphEntities[graphPath].nodes[movedId].parameterGroups).toBe(
    canonical.parameterGroups,
  );
  expect(reordered.projection.nodes[2].parameterGroups[0].key).toBe("second");
  expect(reordered.projection.nodes[2].parameterGroups[1].parameters[0].key).toBe("b");

  const published = useResourceStore.getState().sessions[graphPath].projection;
  const sharedDelta = produce({ ...fresh, projection: published }, (draft) => {
    draft.editing.version.revision = "4";
    const node = draft.projection.nodes[2];
    (node.parameterGroups[0].parameters[0].value as ReturnType<typeof opaque>).nested.key = "delta";
    (node.ports[inputIndex].input!.literalOverride as ReturnType<typeof opaque>).nested.key =
      "delta";
  });
  store.installGraphSession(graphPath, sharedDelta);
  expect(useResourceStore.getState().sessions[graphPath].projection).toBe(sharedDelta.projection);

  const freshValueChange = structuredClone(sharedDelta);
  const changedParameter = freshValueChange.projection.nodes[2].parameterGroups[0].parameters[0];
  (changedParameter.value as ReturnType<typeof opaque>).nested.key = "fresh";
  const changedInput = freshValueChange.projection.nodes[2].ports[inputIndex].input!;
  (changedInput.protocolDefault as ReturnType<typeof opaque>).nested.key = "fresh";
  store.installGraphSession(graphPath, freshValueChange);
  const finalProjection = useResourceStore.getState().sessions[graphPath].projection;
  const finalNode = finalProjection.nodes[2];
  const finalValue = finalNode.parameterGroups[0].parameters[0].value as ReturnType<typeof opaque>;
  const previousValue = sharedDelta.projection.nodes[2].parameterGroups[0].parameters[0]
    .value as ReturnType<typeof opaque>;
  expect(Object.prototype.hasOwnProperty.call(finalValue, "__proto__")).toBe(true);
  expect(Object.getPrototypeOf(finalValue)).toBe(Object.prototype);
  expect(finalValue.__proto__).toBe(previousValue.__proto__);
  expect(finalValue.nested).toBe((changedParameter.value as ReturnType<typeof opaque>).nested);
  const finalInput = finalNode.ports[inputIndex].input!;
  const previousInput = sharedDelta.projection.nodes[2].ports[inputIndex].input!;
  expect(finalInput.literalOverride).toBe(previousInput.literalOverride);
  expect((finalInput.protocolDefault as ReturnType<typeof opaque>).__proto__).toBe(
    (previousInput.protocolDefault as ReturnType<typeof opaque>).__proto__,
  );
  expect(previousValue.nested.key).toBe("delta");
  store.installGraphSession(graphPath, structuredClone(freshValueChange));
  expect(useResourceStore.getState().sessions[graphPath].projection).toBe(finalProjection);

  // These ordinary roots exercise draft traversal before a nested reserved-key fallback.
  const ordinary = structuredClone(freshValueChange);
  const ordinaryNode = ordinary.projection.nodes[2];
  ordinaryNode.parameterGroups[0].parameters[0].value = makeGraphProjectionJsonFixture("parameter");
  ordinaryNode.ports[inputIndex].input!.literalOverride = makeGraphProjectionJsonFixture("literal");
  ordinaryNode.ports[inputIndex].input!.protocolDefault = makeGraphProjectionJsonFixture("default");
  store.installGraphSession(graphPath, ordinary);
  const ordinaryProjection = useResourceStore.getState().sessions[graphPath].projection;
  type JsonFixture = ReturnType<typeof makeGraphProjectionJsonFixture>;
  const values = (projection: typeof ordinaryProjection): JsonFixture[] => {
    const node = projection.nodes[2];
    return [
      node.parameterGroups[0].parameters[0].value,
      node.ports[inputIndex].input!.literalOverride,
      node.ports[inputIndex].input!.protocolDefault,
    ] as JsonFixture[];
  };
  const ordinaryDelta = produce({ ...ordinary, projection: ordinaryProjection }, (draft) => {
    values(draft.projection).forEach(changeGraphProjectionJsonFixture);
  });
  store.installGraphSession(graphPath, ordinaryDelta);
  expect(useResourceStore.getState().sessions[graphPath].projection).toBe(ordinaryDelta.projection);

  // Reinstall the same baseline before comparing a fully materialized candidate.
  store.installGraphSession(graphPath, ordinary);
  const changed = structuredClone(ordinary);
  values(changed.projection).forEach(changeGraphProjectionJsonFixture);
  store.installGraphSession(graphPath, changed);
  const sharedProjection = useResourceStore.getState().sessions[graphPath].projection;
  expect(sharedProjection).toEqual(changed.projection);
  values(sharedProjection).forEach((value, index) => {
    const before = values(ordinaryProjection)[index];
    const incoming = values(changed.projection)[index];
    expect(value.left[0]).toBe(incoming.left[0]);
    expect(value.left[1]).toBe(before.left[1]);
    expect(value.right).toBe(before.right);
    expect(value.metadata.__proto__).toBe(incoming.metadata.__proto__);
    expect(value.metadata.constructor.keep).toBe(before.metadata.constructor.keep);
    expect(value.metadata.constructor.change).toBe(incoming.metadata.constructor.change);
    expect(Object.prototype.hasOwnProperty.call(value.metadata, "__proto__")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(value.metadata, "constructor")).toBe(true);
    expect(Object.getPrototypeOf(value.metadata)).toBe(Object.prototype);
    expect(value.metadata).not.toHaveProperty("enabled");
    expect(value.metadata).toHaveProperty("added", { key: "added" });
    expect(before.metadata.__proto__.items).toEqual(["first", "second"]);
    expect(before.metadata.constructor.change.key).toBe("old");
    expect(before.metadata.enabled).toBe(true);
  });
  store.installGraphSession(graphPath, structuredClone(changed));
  expect(useResourceStore.getState().sessions[graphPath].projection).toBe(sharedProjection);
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
  const store = useResourceStore.getState();
  store.installGraphSession(graphPath, session, { mode: "load" });
  const before = useResourceStore.getState().graphEntities[graphPath];
  const beforeProjection = useResourceStore.getState().sessions[graphPath].projection;
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
  store.installGraphSession(graphPath, structuredClone(reordered));
  const order = useResourceStore.getState().graphEntities[graphPath];
  const orderProjection = useResourceStore.getState().sessions[graphPath].projection;
  expect(order.graphNodes).toEqual([...before.graphNodes].reverse());
  for (const [index, node] of orderProjection.nodes.entries())
    expect(node).toBe(beforeProjection.nodes[beforeProjection.nodes.length - index - 1]);
  expect(order.nodes).toBe(before.nodes);
  expect(order.pins).toBe(before.pins);
  expect(order.connections).toBe(before.connections);
  expect(order.pinConnections[out]).toEqual(["second", "first"]);
  expect(order.pinConnections[leftIn]).toBe(before.pinConnections[leftIn]);
  expect(order.blockedConnectionIds).toEqual({ second: true });

  const rerouted = produce(reordered, (draft) => {
    draft.editing.version.revision = "2";
    draft.projection.nodes[0].ports.reverse();
    draft.projection.connections[0].input = left.inputAddress;
    draft.projection.diagnostics[0].blocking = false;
    draft.projection.hasBlockingDiagnostics = false;
    draft.projection.outcome = { type: "success" };
  });
  store.installGraphSession(graphPath, structuredClone(rerouted));
  const route = useResourceStore.getState().graphEntities[graphPath];
  const routeProjection = useResourceStore.getState().sessions[graphPath].projection;
  for (const [index, port] of routeProjection.nodes[0].ports.entries())
    expect(port).toBe(
      orderProjection.nodes[0].ports[orderProjection.nodes[0].ports.length - index - 1],
    );
  expect(route.pins).toBe(order.pins);
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
  store.installGraphSession(graphPath, replaced);
  const after = useResourceStore.getState().graphEntities[graphPath];
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
