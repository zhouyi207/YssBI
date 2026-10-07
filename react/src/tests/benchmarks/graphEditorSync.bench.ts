import { parseEditorGraphProjectionDto } from "@/shared/types/domain/editorProjectionParser";
import {
  makeGraphConstantFixture,
  makeGraphEditorSession,
  makeGraphProjectionJsonFixture,
  changeGraphProjectionJsonFixture,
} from "@/tests/helpers/editorProjectionFixtures";
import { bench, expect, vi } from "vitest";
import { produce } from "immer";
import { parseGraphDocumentDto } from "@/shared/types/dto/editorMutationWireParser";
import { freezePublishedValue, isPublishedValue } from "@/shared/types/deepReadonly";
import { portAddressKey } from "@/shared/types/domain/portAddressKey";
import { invoke } from "@tauri-apps/api/core";
import projectionFixture from "@/tests/fixtures/node-system-contracts/editor-projection.json";
import {
  invokeGraphSync,
  clearGraphSyncBaselines,
  type GraphSyncReply,
} from "@/services/nodeSystem/graphEditorSync";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { shareProjection } from "@/features/core/state/readProjection";
import { shareGraphConstants } from "@/features/core/dataStore/graphConstantProjection";
import { prepareGraphSessions } from "@/features/core/dataStore/graphProjection";
import {
  shareGraphConstantsWithImmerReference,
  makeDynamicKeyConstantFixture,
  changeDynamicKeyConstantFixture,
} from "./graphConstantSharingReference";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const cases = new Map<
  string,
  { wire: string; nodeId: string; revision: number; snapshotBytes: number }
>();

vi.mocked(invoke).mockImplementation(async (_command, raw) => {
  const args = raw as {
    projectInstanceId: string;
    graphPath: string;
    locale: string;
    cursor: string | null;
  };
  const fixture = cases.get(args.projectInstanceId)!;
  if (!args.cursor) {
    const snapshot = JSON.parse(fixture.wire);
    snapshot.update.data.editing.version.revision = String(fixture.revision);
    return snapshot;
  }
  fixture.revision++;
  const x = fixture.revision % 2;
  return JSON.parse(
    JSON.stringify({
      projectInstanceId: args.projectInstanceId,
      graphPath: args.graphPath,
      locale: args.locale,
      changed: true,
      resourceRevision: null,
      functionEditorProjection: null,
      update: {
        kind: "delta",
        baseCursor: args.cursor,
        cursor: String(fixture.revision),
        snapshotBytes: fixture.snapshotBytes,
        changes: [
          { kind: "set", path: ["document", "nodes", fixture.nodeId, "position", "x"], value: x },
          { kind: "set", path: ["projection", "nodes", "0", "position", "x"], value: x },
          {
            kind: "set",
            path: ["editing", "version", "revision"],
            value: String(fixture.revision),
          },
          { kind: "set", path: ["editing", "dirty"], value: true },
          { kind: "set", path: ["editing", "canUndo"], value: true },
        ],
      },
    }),
  );
});

for (const count of [100, 1000, 5000]) {
  const binding = {
    projectInstanceId: `bench-${count}`,
    graphPath: `events/Bench-${count}.yssbi-event`,
    locale: "en-US",
  };
  const projection = structuredClone(projectionFixture);
  projection.graphPath = binding.graphPath;
  projection.basis.graphPath = binding.graphPath;
  const template = JSON.stringify(projection.nodes[0]);
  const templateId = projection.nodes[0].nodeId;
  projection.nodes = [];
  const document = {
    nodes: {} as Record<string, unknown>,
    port_bindings: [],
    connections: {},
    input_states: [],
  };
  let firstId = "";
  for (let index = 0; index < count; index++) {
    const id = `00000000-0000-0000-0000-${(index + 100).toString(16).padStart(12, "0")}`;
    if (!firstId) firstId = id;
    const node = JSON.parse(template.split(templateId).join(id));
    node.graphPath = binding.graphPath;
    node.position = { x: 0, y: index * 100 };
    projection.nodes.push(node);
    document.nodes[id] = {
      id,
      node_type: node.nodeTypeId,
      position: node.position,
      parameters: {},
      user_label: null,
    };
  }
  const response = {
    ...binding,
    changed: false,
    resourceRevision: null,
    functionEditorProjection: null,
    update: {
      kind: "snapshot",
      cursor: "initial",
      snapshotBytes: 0,
      data: {
        resultState: makeGraphEditorSession(parseEditorGraphProjectionDto(projection)).resultState,

        document,
        projection,
        editing: {
          version: { sessionId: "00000000-0000-0000-0000-000000000001", revision: "0" },
          dirty: false,
          canUndo: false,
          canRedo: false,
        },
      },
    },
  };
  response.update.snapshotBytes = Buffer.byteLength(JSON.stringify(response.update.data));
  const wire = JSON.stringify(response);
  cases.set(binding.projectInstanceId, {
    wire,
    nodeId: firstId,
    revision: 0,
    snapshotBytes: response.update.snapshotBytes,
  });
  const install = async () => {
    const reply = await invokeGraphSync("hydrate_editor_graph", binding, binding);
    useResourceStore.getState().installGraphSession(binding.graphPath, reply.data);
  };
  bench(
    `${count} nodes: snapshot parse and adoption`,
    async () => {
      clearGraphSyncBaselines();
      useResourceStore.getState().removeGraphSession(binding.graphPath);
      await install();
    },
    {
      time: 500,
      iterations: 30,
      warmupTime: 100,
      setup: (task) => {
        // Mock results otherwise retain every historical snapshot and distort GC costs.
        task.opts.afterEach = () => {
          vi.mocked(invoke).mockClear();
        };
      },
    },
  );
  bench(
    `${count} nodes: snapshot parse`,
    async () => {
      await invokeGraphSync("hydrate_editor_graph", binding, binding);
    },
    {
      time: 500,
      iterations: 30,
      warmupTime: 100,
      setup: (task) => {
        task.opts.beforeEach = () => {
          clearGraphSyncBaselines();
          useResourceStore.getState().removeGraphSession(binding.graphPath);
        };
        task.opts.afterEach = () => {
          vi.mocked(invoke).mockClear();
        };
      },
    },
  );
  let preparedSnapshot: GraphSyncReply;
  bench(
    `${count} nodes: snapshot adoption`,
    () => {
      useResourceStore.getState().installGraphSession(binding.graphPath, preparedSnapshot.data);
    },
    {
      time: 500,
      iterations: 30,
      warmupTime: 100,
      setup: (task) => {
        task.opts.beforeEach = async () => {
          clearGraphSyncBaselines();
          useResourceStore.getState().removeGraphSession(binding.graphPath);
          preparedSnapshot = await invokeGraphSync("hydrate_editor_graph", binding, binding);
        };
        task.opts.afterEach = () => {
          vi.mocked(invoke).mockClear();
        };
      },
    },
  );
  bench(`${count} nodes: delta parse and adoption`, install, {
    time: 500,
    iterations: 30,
    warmupTime: 100,
    setup: async (task) => {
      await install();
      task.opts.afterEach = () => {
        vi.mocked(invoke).mockClear();
      };
    },
  });
  bench(
    `${count} nodes: delta parse`,
    async () => {
      await invokeGraphSync("hydrate_editor_graph", binding, binding);
    },
    {
      time: 500,
      iterations: 30,
      warmupTime: 100,
      setup: async (task) => {
        await install();
        task.opts.afterEach = () => {
          vi.mocked(invoke).mockClear();
        };
      },
    },
  );
  let prepared: GraphSyncReply;
  let beforeDelta: ReturnType<typeof useResourceStore.getState>;
  bench(
    `${count} nodes: delta adoption`,
    () => {
      useResourceStore.getState().installGraphSession(binding.graphPath, prepared.data);
    },
    {
      time: 500,
      iterations: 30,
      warmupTime: 100,
      setup: async (task) => {
        await install();
        task.opts.afterEach = () => {
          const after = useResourceStore.getState();
          const previousBucket = beforeDelta.graphEntities[binding.graphPath];
          const nextBucket = after.graphEntities[binding.graphPath];
          expect(after.sessions[binding.graphPath].projection).toBe(prepared.data.projection);
          expect(isPublishedValue(prepared.data.projection)).toBe(true);
          expect(nextBucket.nodes[firstId].position).toBe(
            prepared.data.projection.nodes[0].position,
          );
          expect(nextBucket.nodes[previousBucket.graphNodes[1]]).toBe(
            previousBucket.nodes[previousBucket.graphNodes[1]],
          );
          expect(nextBucket.pins).toBe(previousBucket.pins);
          expect(nextBucket.graphNodes).toBe(previousBucket.graphNodes);
          vi.mocked(invoke).mockClear();
        };
        // Vitest exposes task hooks through setup; tinybench excludes them from each sample.
        task.opts.beforeEach = async () => {
          beforeDelta = useResourceStore.getState();
          prepared = await invokeGraphSync("hydrate_editor_graph", binding, binding);
        };
      },
    },
  );

  // Keep the original one-port load/delta fixtures comparable. Readoption uses two ports
  // per node so that the reordered snapshot really changes both node and port order.
  const replacementProjection = parseEditorGraphProjectionDto(structuredClone(projection));
  for (const node of replacementProjection.nodes) {
    const port = structuredClone(node.ports[0]);
    if (port.address.kind !== "declared") throw new Error("Expected a declared fixture port");
    port.address.portKey += "-second";
    node.ports.push(port);
  }
  const replacementSession = makeGraphEditorSession(replacementProjection);
  replacementSession.document = parseGraphDocumentDto(structuredClone(document));
  replacementSession.editing = structuredClone(response.update.data.editing);

  for (const scenario of ["equal", "one-node move", "node and port reorder"] as const) {
    const replacementBinding = {
      ...binding,
      projectInstanceId: `${binding.projectInstanceId}-${scenario}`,
    };
    const baseResponse = {
      ...response,
      ...replacementBinding,
      update: { ...response.update, data: replacementSession },
    };
    baseResponse.update.snapshotBytes = Buffer.byteLength(JSON.stringify(replacementSession));
    const baseWire = JSON.stringify(baseResponse);
    const nextResponse = structuredClone(baseResponse);
    const nextSession = nextResponse.update.data;
    if (scenario !== "equal") {
      nextSession.editing.version.revision = "1";
      nextResponse.changed = true;
      if (scenario === "one-node move") {
        nextSession.document.nodes[firstId].position.x = 1;
        nextSession.projection.nodes[0].position.x = 1;
      } else {
        nextSession.projection.nodes.reverse();
        for (const node of nextSession.projection.nodes) node.ports.reverse();
      }
    }
    nextResponse.update.snapshotBytes = Buffer.byteLength(JSON.stringify(nextSession));
    const nextWire = JSON.stringify(nextResponse);
    const fixture = {
      wire: baseWire,
      nodeId: firstId,
      revision: 0,
      snapshotBytes: nextResponse.update.snapshotBytes,
    };
    cases.set(replacementBinding.projectInstanceId, fixture);

    for (const phase of ["sync", "adoption", "sync and adoption"] as const) {
      let base: ReturnType<typeof useResourceStore.getState>;
      let baseDocument: GraphSyncReply["data"]["document"];
      let received: GraphSyncReply;
      const receive = () =>
        invokeGraphSync("hydrate_editor_graph", replacementBinding, replacementBinding);
      const adopt = (): void => {
        useResourceStore.getState().installGraphSession(binding.graphPath, received.data);
      };
      const verify = (full: boolean) => {
        const before = base.sessions[binding.graphPath];
        const beforeBucket = base.graphEntities[binding.graphPath];
        // A cached candidate would hide full-snapshot reconciliation costs.
        expect(received.data.document).not.toBe(baseDocument);
        expect(received.data.projection).not.toBe(before.projection);
        expect(received.data.resultState).not.toBe(base.resultStates[binding.graphPath]);
        expect(isPublishedValue(received.data.projection)).toBe(true);
        if (full) {
          expect(received.data.projection.nodes).toHaveLength(count);
          expect(received.data.resultState.outputs).toHaveLength(count * 2);
          expect(received.data.editing.version.revision).toBe(scenario === "equal" ? "0" : "1");
          if (scenario === "one-node move") {
            expect(received.data.document.nodes[firstId].position.x).toBe(1);
            expect(received.data.projection.nodes[0].position.x).toBe(1);
          } else if (scenario === "node and port reorder") {
            expect(received.data.projection.nodes.map((node) => node.nodeId)).toEqual(
              [...beforeBucket.graphNodes].reverse(),
            );
            for (const node of received.data.projection.nodes)
              expect(node.ports.map((port) => portAddressKey(port.address))).toEqual(
                [...beforeBucket.nodes[node.nodeId].pinIds].reverse(),
              );
          }
        }
        const after = useResourceStore.getState();
        if (phase === "sync" || scenario === "equal") {
          expect(after).toBe(base);
          return;
        }
        const afterBucket = after.graphEntities[binding.graphPath];
        expect(after.sessions[binding.graphPath].version.revision).toBe("1");
        expect(afterBucket).not.toBe(beforeBucket);
        expect(after.resultStates[binding.graphPath]).toBe(base.resultStates[binding.graphPath]);
        expect(afterBucket.pins).toBe(beforeBucket.pins);
        expect(afterBucket.connections).toBe(beforeBucket.connections);
        expect(afterBucket.pinConnections).toBe(beforeBucket.pinConnections);
        if (scenario === "one-node move") {
          expect(afterBucket.nodes[firstId].position.x).toBe(1);
          expect(beforeBucket.nodes[firstId].position.x).toBe(0);
          expect(afterBucket.graphNodes).toBe(beforeBucket.graphNodes);
          if (full) {
            for (const id of beforeBucket.graphNodes.slice(1))
              expect(afterBucket.nodes[id]).toBe(beforeBucket.nodes[id]);
          }
        } else if (full) {
          expect(afterBucket.graphNodes).toEqual([...beforeBucket.graphNodes].reverse());
          for (const id of beforeBucket.graphNodes) {
            expect(afterBucket.nodes[id].pinIds).toEqual(
              [...beforeBucket.nodes[id].pinIds].reverse(),
            );
            expect(afterBucket.nodes[id].position).toBe(beforeBucket.nodes[id].position);
          }
          const first = received.data.projection.nodes[0];
          expect(afterBucket.nodes[first.nodeId].pinIds).toEqual(
            first.ports.map((port) => portAddressKey(port.address)),
          );
        }
      };
      bench(
        `${count} nodes: existing graph ${scenario} snapshot ${phase}`,
        phase === "adoption"
          ? adopt
          : async () => {
              received = await receive();
              if (phase === "sync and adoption") adopt();
            },
        {
          time: 500,
          iterations: 30,
          warmupTime: 100,
          setup: async (task) => {
            clearGraphSyncBaselines();
            useResourceStore.getState().clear();
            fixture.wire = baseWire;
            fixture.revision = 0;
            received = await receive();
            baseDocument = received.data.document;
            adopt();
            base = useResourceStore.getState();
            fixture.wire = nextWire;
            fixture.revision = Number(nextSession.editing.version.revision);
            // Validate the complete scenario once outside measured samples.
            clearGraphSyncBaselines();
            received = await receive();
            if (phase !== "sync") adopt();
            verify(true);
            task.opts.beforeEach = async () => {
              useResourceStore.setState(base, true);
              clearGraphSyncBaselines();
              vi.mocked(invoke).mockClear();
              // Sync includes JSON materialization, freezing and validation. Adoption
              // measures only installation of that freshly received, validated snapshot.
              if (phase === "adoption") received = await receive();
            };
            task.opts.afterEach = () => {
              verify(false);
              vi.mocked(invoke).mockClear();
            };
          },
        },
      );
    }
  }
}

// This isolates the constants boundary. The graph snapshot fixtures above keep their
// original empty constants; they are not evidence for nested constant reconciliation.
// These original fixtures have ordinary keys. The reference now delegates dictionaries
// with an own __proto__ to the existing helper; the separate dynamic-key cases cover it.
const constantsCount = 500;
const constantIds = Array.from(
  { length: constantsCount },
  (_, index) => `00000000-0000-0000-0000-${(index + 1).toString(16).padStart(12, "0")}`,
);
const constants = Object.fromEntries(constantIds.map((id) => [id, makeGraphConstantFixture(id)]));
const constantsDocument = {
  constants,
  nodes: {},
  port_bindings: [],
  connections: {},
  input_states: [],
};
const prepareConstants = (wire: string) => {
  const document = JSON.parse(wire);
  freezePublishedValue(document);
  return parseGraphDocumentDto(document).constants!;
};
const previousConstants = prepareConstants(JSON.stringify(constantsDocument));
const firstConstantId = constantIds[0];
const lastConstantId = constantIds[constantIds.length - 1];
type ConstantValue = (typeof constants)[string]["dataValue"];

for (const scenario of ["equal", "name-only", "nested change"] as const) {
  const document = structuredClone(constantsDocument);
  if (scenario === "name-only") document.constants[firstConstantId].name = "Renamed";
  if (scenario === "nested change")
    document.constants[firstConstantId].dataValue.Object.left.List[0].Integer = "999";
  const wire = JSON.stringify(document);
  for (const [strategy, share] of [
    ["manual", shareProjection],
    ["immer reference", shareGraphConstantsWithImmerReference],
    ["typed immer", shareGraphConstants],
  ] as const) {
    let next: typeof previousConstants;
    let result: typeof previousConstants;
    const reconcile = () => {
      result = share(previousConstants, next)!;
      // Every strategy pays the same publication-proof step used by the store.
      freezePublishedValue(result);
    };
    const verify = () => {
      expect(next).not.toBe(previousConstants);
      expect(next[firstConstantId].dataValue).not.toBe(
        previousConstants[firstConstantId].dataValue,
      );
      expect(isPublishedValue(next)).toBe(true);
      expect(isPublishedValue(result)).toBe(true);
      expect(result).toEqual(next);
      expect(result[lastConstantId]).toBe(previousConstants[lastConstantId]);
      if (scenario === "equal") expect(result).toBe(previousConstants);
      else {
        expect(result).not.toBe(previousConstants);
        expect(result[firstConstantId]).not.toBe(previousConstants[firstConstantId]);
        expect(result[firstConstantId].tags).toBe(previousConstants[firstConstantId].tags);
        if (scenario === "name-only") {
          expect(result[firstConstantId].dataValue).toBe(
            previousConstants[firstConstantId].dataValue,
          );
        } else {
          const value = result[firstConstantId].dataValue as ConstantValue;
          const before = previousConstants[firstConstantId].dataValue as ConstantValue;
          const incoming = next[firstConstantId].dataValue as ConstantValue;
          expect(value.Object.left.List[0]).toBe(incoming.Object.left.List[0]);
          expect(value.Object.left.List[1]).toBe(before.Object.left.List[1]);
          expect(value.Object.right).toBe(before.Object.right);
          expect(value.Object.metadata).toBe(before.Object.metadata);
          expect(before.Object.left.List[0].Integer).toBe("0");
        }
      }
    };
    bench(`${constantsCount} constants: ${scenario} ${strategy}`, reconcile, {
      time: 0,
      iterations: 30,
      warmupTime: 100,
      setup: (task) => {
        next = prepareConstants(wire);
        reconcile();
        verify();
        task.opts.beforeEach = () => {
          // JSON materialization, freezing and full DTO validation are not timed.
          next = prepareConstants(wire);
        };
        task.opts.afterEach = verify;
      },
    });
  }
}

// A full legal-input comparison of the constants boundary, not an isolated JSON-helper cost.
// The reference retains one Immer transaction and falls back only at unsafe dictionaries.
const dynamicConstants = Object.fromEntries(
  constantIds.map((id) => [id, makeDynamicKeyConstantFixture(id)]),
);
const dynamicDocument = { ...constantsDocument, constants: dynamicConstants };
const previousDynamicConstants = prepareConstants(
  JSON.stringify(dynamicDocument),
) as typeof dynamicConstants;
type DynamicConstantValue = (typeof dynamicConstants)[string]["dataValue"];

for (const scenario of ["equal", "name-only", "nested change", "shared delta"] as const) {
  const document = structuredClone(dynamicDocument);
  if (scenario === "name-only") document.constants[firstConstantId].name = "Renamed";
  if (scenario === "nested change")
    changeDynamicKeyConstantFixture(document.constants[firstConstantId]);
  const wire = JSON.stringify(document);
  const prepare = () => {
    if (scenario !== "shared delta") return prepareConstants(wire) as typeof dynamicConstants;
    const constants = produce(previousDynamicConstants, (draft) => {
      changeDynamicKeyConstantFixture(draft[firstConstantId]);
    });
    freezePublishedValue(constants);
    parseGraphDocumentDto({ ...dynamicDocument, constants });
    return constants;
  };
  for (const [strategy, share] of [
    ["typed production", shareGraphConstants],
    ["immer safe-dict reference", shareGraphConstantsWithImmerReference],
  ] as const) {
    let next: typeof dynamicConstants;
    let result: NonNullable<ReturnType<typeof shareGraphConstants>>;
    const reconcile = () => {
      result = share(previousDynamicConstants, next)!;
      freezePublishedValue(result);
    };
    const verify = () => {
      expect(next).not.toBe(previousDynamicConstants);
      expect(isPublishedValue(next)).toBe(true);
      expect(isPublishedValue(result)).toBe(true);
      expect(result).toEqual(next);
      expect(result[lastConstantId]).toBe(previousDynamicConstants[lastConstantId]);
      if (scenario === "equal") expect(result).toBe(previousDynamicConstants);
      else if (scenario === "name-only") {
        expect(result[firstConstantId].dataValue).toBe(
          previousDynamicConstants[firstConstantId].dataValue,
        );
        expect(result[firstConstantId].tags).toBe(previousDynamicConstants[firstConstantId].tags);
      } else {
        if (scenario === "shared delta") expect(result).toBe(next);
        const value = (result[firstConstantId].dataValue as DynamicConstantValue).Object;
        const before = previousDynamicConstants[firstConstantId].dataValue.Object;
        const incoming = next[firstConstantId].dataValue.Object;
        expect(value.left.List[0]).toBe(incoming.left.List[0]);
        expect(value.left.List[1]).toBe(before.left.List[1]);
        expect(value.right).toBe(before.right);
        expect(value.metadata.Object.__proto__).toBe(incoming.metadata.Object.__proto__);
        expect(value.metadata.Object.constructor.Object.keep).toBe(
          before.metadata.Object.constructor.Object.keep,
        );
        expect(value.metadata.Object.constructor.Object.change).toBe(
          incoming.metadata.Object.constructor.Object.change,
        );
        expect(Object.prototype.hasOwnProperty.call(value.metadata.Object, "__proto__")).toBe(true);
        expect(Object.getPrototypeOf(value.metadata.Object)).toBe(Object.prototype);
        expect(value.metadata.Object).not.toHaveProperty("enabled");
        expect(before.metadata.Object.__proto__.List[0].Integer).toBe("40");
        expect(before.metadata.Object).toHaveProperty("enabled");
      }
    };
    bench(`${constantsCount} constants with dynamic keys: ${scenario} ${strategy}`, reconcile, {
      time: 0,
      iterations: 30,
      warmupTime: 100,
      setup: (task) => {
        next = prepare();
        reconcile();
        verify();
        task.opts.beforeEach = () => {
          next = prepare();
        };
        task.opts.afterEach = verify;
      },
    });
  }
}

// All three opaque projection fields carry ordinary JSON, independently of constants.
// Compare candidates through the existing preparation owner, including its unchanged
// validation, entity derivation and publication freeze work, not a private helper API.
const jsonNodeCount = 500;
const jsonGraphPath = "events/Projection-json.yssbi-event";
const jsonProjection = parseEditorGraphProjectionDto(structuredClone(projectionFixture));
jsonProjection.graphPath = jsonGraphPath;
jsonProjection.basis.graphPath = jsonGraphPath;
const jsonNodeTemplate = jsonProjection.nodes[0];
jsonProjection.nodes = Array.from({ length: jsonNodeCount }, (_, index) => {
  const node = structuredClone(jsonNodeTemplate);
  node.nodeId = `00000000-0000-0000-0000-${(index + 1).toString(16).padStart(12, "0")}`;
  node.nodeTypeId = "tests.projected-json";
  node.graphPath = jsonGraphPath;
  node.position.y = index;
  node.parameterGroups[0].parameters = [
    {
      ...node.parameterGroups[0].parameters[0],
      key: "json",
      editor: { kind: "auto" },
      valueType: { kind: "Object" },
      value: makeGraphProjectionJsonFixture("parameter"),
    },
  ];
  const port = node.ports[0];
  port.address = { kind: "declared", nodeId: node.nodeId, portKey: "input" };
  port.direction = "input";
  port.typeState = { status: "exact", display: "Object", dataType: { kind: "Object" } };
  port.input = {
    literalOverride: makeGraphProjectionJsonFixture("literal"),
    protocolDefault: makeGraphProjectionJsonFixture("default"),
    effective: "literal",
  };
  return node;
});
const jsonSession = makeGraphEditorSession(jsonProjection);
freezePublishedValue(jsonSession);
parseEditorGraphProjectionDto(jsonSession.projection);
parseGraphDocumentDto(jsonSession.document);
const jsonBase = prepareGraphSessions(
  [{ graphPath: jsonGraphPath, session: jsonSession }],
  undefined,
  {
    sessions: {},
    graphEntities: {},
    resultStates: {},
  },
).state;
type ProjectionJsonValue = ReturnType<typeof makeGraphProjectionJsonFixture>;
const projectionJsonValues = (projection: typeof jsonProjection): ProjectionJsonValue[] => {
  const node = projection.nodes[0];
  return [
    node.parameterGroups[0].parameters[0].value,
    node.ports[0].input!.literalOverride,
    node.ports[0].input!.protocolDefault,
  ] as ProjectionJsonValue[];
};

for (const scenario of ["equal", "nested change", "shared delta"] as const) {
  const candidate = structuredClone(jsonProjection);
  if (scenario !== "equal")
    projectionJsonValues(candidate).forEach(changeGraphProjectionJsonFixture);
  const wire = JSON.stringify(candidate);
  const prepare = () => {
    const projection =
      scenario === "shared delta"
        ? produce(jsonProjection, (draft) => {
            projectionJsonValues(draft).forEach(changeGraphProjectionJsonFixture);
          })
        : (JSON.parse(wire) as typeof jsonProjection);
    freezePublishedValue(projection);
    parseEditorGraphProjectionDto(projection);
    return { ...jsonSession, projection };
  };
  let next: ReturnType<typeof prepare>;
  let prepared: ReturnType<typeof prepareGraphSessions>;
  const reconcile = () => {
    prepared = prepareGraphSessions(
      [{ graphPath: jsonGraphPath, session: next }],
      undefined,
      jsonBase,
    );
  };
  const verify = (full: boolean) => {
    const projection = prepared.state.sessions[jsonGraphPath].projection;
    expect(next.projection).not.toBe(jsonProjection);
    expect(isPublishedValue(next.projection)).toBe(true);
    expect(isPublishedValue(projection)).toBe(true);
    expect(projection.nodes).toHaveLength(jsonNodeCount);
    if (full) expect(projection).toEqual(next.projection);
    if (scenario === "equal") {
      expect(prepared.state).toBe(jsonBase);
      return;
    }
    if (scenario === "shared delta") expect(projection).toBe(next.projection);
    expect(projection.nodes[jsonNodeCount - 1]).toBe(jsonProjection.nodes[jsonNodeCount - 1]);
    expect(prepared.state.graphEntities[jsonGraphPath].graphNodes).toBe(
      jsonBase.graphEntities[jsonGraphPath].graphNodes,
    );
    projectionJsonValues(projection).forEach((value, index) => {
      const before = projectionJsonValues(jsonProjection)[index];
      const incoming = projectionJsonValues(next.projection)[index];
      expect(value).not.toBe(before);
      expect(value.nested.key).toBe("changed");
      expect(value.left[0]).toBe(incoming.left[0]);
      expect(value.left[1]).toBe(before.left[1]);
      expect(value.right).toBe(before.right);
      expect(value.metadata.__proto__).toBe(incoming.metadata.__proto__);
      expect(value.metadata.__proto__.items).toEqual(["second", "first"]);
      expect(value.metadata.constructor.keep).toBe(before.metadata.constructor.keep);
      expect(value.metadata.constructor.change).toBe(incoming.metadata.constructor.change);
      expect(value.metadata.constructor.change.key).toBe("changed");
      expect(Object.prototype.hasOwnProperty.call(value.metadata, "__proto__")).toBe(true);
      expect(Object.prototype.hasOwnProperty.call(value.metadata, "constructor")).toBe(true);
      expect(Object.getPrototypeOf(value.metadata)).toBe(Object.prototype);
      expect(value.metadata).not.toHaveProperty("enabled");
      expect(value.metadata).toHaveProperty("added", { key: "added" });
      expect(before.metadata.__proto__.items).toEqual(["first", "second"]);
      expect(before.metadata.constructor.change.key).toBe("old");
      expect(before.metadata.enabled).toBe(true);
    });
  };
  bench(`${jsonNodeCount} nodes with JSON values: ${scenario} preparation`, reconcile, {
    time: 0,
    iterations: 30,
    warmupTime: 100,
    setup: (task) => {
      next = prepare();
      reconcile();
      verify(true);
      task.opts.beforeEach = () => {
        next = prepare();
      };
      task.opts.afterEach = () => verify(false);
    },
  });
}
