import { beforeEach, describe, expect, it, vi } from "vitest";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import { portAddressKey } from "@/features/domain/editorProjection";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useExecutionStore } from "@/features/core/execution";
import { executeCommand, executeCommandOutcome, executeCommandWithResult } from "./commandExecutor";
import { ensureGraphDraftPortRegistered } from "@/features/application/graphDraft/registerGraphDraftPort";

const applyGraphDraftMutation = vi.hoisted(() => vi.fn());

vi.mock("@/features/application/graphDraft/graphDraftCoordinator", () => ({
  applyGraphDraftMutation,
}));
vi.mock(
  "@/features/application/editorProjection/graphProjectionCoordinator",
  async (importOriginal) => ({
    ...(await importOriginal<
      typeof import("@/features/application/editorProjection/graphProjectionCoordinator")
    >()),
    currentProjectionLocale: () => "en-US",
    hydrateGraphProjection: vi.fn(async () => true),
  }),
);

ensureGraphDraftPortRegistered();

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

const graphPath = "events/main.yssbi-event";

function installProjection() {
  const fixture = makeEditorProjectionFixture({ graphPath, sourceRevision: 3 });
  fixture.projection.nodes[0].ports[1] = {
    ...fixture.projection.nodes[0].ports[1],
    address: {
      kind: "instance",
      nodeId: "local-node",
      templateKey: "inputs",
      instanceId: "00000000-0000-0000-0000-000000000011",
    },
    canRemove: true,
  };
  fixture.projection.connections[0] = {
    ...fixture.projection.connections[0],
    input: fixture.projection.nodes[0].ports[1].address,
  };
  const inputAddress = fixture.projection.nodes[0].ports[1].address;
  if (inputAddress.kind !== "instance") throw new Error("fixture input must be an instance port");
  useGraphProjectionStore.getState().replaceProjection(graphPath, fixture.projection, 1);
  return {
    outputKey: portAddressKey(fixture.projection.nodes[0].ports[0].address),
    inputKey: portAddressKey(fixture.projection.nodes[0].ports[1].address),
    outputAddress: fixture.projection.nodes[0].ports[0].address,
    inputAddress,
  };
}

describe("forward-only editor commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useGraphProjectionStore.setState({ graphEntities: {} });
  });

  it.each([
    {
      type: "ConnectPins" as const,
      build: (fixture: ReturnType<typeof installProjection>) => ({
        pinA: fixture.inputKey,
        pinB: fixture.outputKey,
      }),
      mutation: (fixture: ReturnType<typeof installProjection>) => ({
        type: "connect",
        payload: { output: fixture.outputAddress, input: fixture.inputAddress, order: null },
      }),
    },
    {
      type: "DisconnectPort" as const,
      build: (fixture: ReturnType<typeof installProjection>) => ({ pinId: fixture.inputKey }),
      mutation: (fixture: ReturnType<typeof installProjection>) => ({
        type: "disconnectPort",
        payload: { address: fixture.inputAddress },
      }),
    },
    {
      type: "DisconnectNode" as const,
      build: () => ({ nodeId: "local-node" }),
      mutation: () => ({ type: "disconnectNode", payload: { nodeId: "local-node" } }),
    },
    {
      type: "DisconnectConnections" as const,
      build: () => ({ connectionIds: ["connection-a", "connection-b"] }),
      mutation: () => ({
        type: "disconnectConnections",
        payload: { connectionIds: ["connection-a", "connection-b"] },
      }),
    },
    {
      type: "MoveConnections" as const,
      build: (fixture: ReturnType<typeof installProjection>) => ({
        sourcePinId: fixture.outputKey,
        targetPinId: fixture.inputKey,
      }),
      mutation: (fixture: ReturnType<typeof installProjection>) => ({
        type: "moveConnections",
        payload: { source: fixture.outputAddress, target: fixture.inputAddress },
      }),
    },
    {
      type: "InsertReroute" as const,
      build: () => ({ connectionId: "edge-1", position: { x: 120, y: 80 } }),
      mutation: () => ({
        type: "insertReroute",
        payload: { connectionId: "edge-1", position: { x: 120, y: 80 } },
      }),
    },
    {
      type: "SetPinValue" as const,
      build: (fixture: ReturnType<typeof installProjection>) => ({
        nodeId: "local-node",
        pinId: fixture.inputKey,
        newValue: 42,
      }),
      mutation: (fixture: ReturnType<typeof installProjection>) => ({
        type: "setLiteral",
        payload: { address: fixture.inputAddress, literal: 42 },
      }),
    },
    {
      type: "DeleteNodes" as const,
      build: () => ({ nodeIds: ["node-a", "node-b"] }),
      mutation: () => ({ type: "deleteNodes", payload: { nodeIds: ["node-a", "node-b"] } }),
    },
    {
      type: "AddPortInstance" as const,
      build: () => ({ nodeId: "local-node", templateKey: "inputs" }),
      mutation: () => ({
        type: "addPortInstance",
        payload: { nodeId: "local-node", templateKey: "inputs", order: null },
      }),
    },
    {
      type: "RemovePortInstance" as const,
      build: (fixture: ReturnType<typeof installProjection>) => ({
        address: fixture.inputAddress,
      }),
      mutation: (fixture: ReturnType<typeof installProjection>) => ({
        type: "removePortInstance",
        payload: { address: fixture.inputAddress },
      }),
    },
  ])(
    "sends $type as exactly one high-level intent without pre-response entity edits",
    async ({ type, build, mutation }) => {
      const fixture = installProjection();
      const before = useGraphProjectionStore.getState().graphEntities[graphPath];
      const pending = deferred<{ status: "applied" }>();
      applyGraphDraftMutation.mockReturnValueOnce(pending.promise);
      const randomId = vi.spyOn(crypto, "randomUUID");

      const command = executeCommand(graphPath, type, build(fixture) as never);

      expect(applyGraphDraftMutation).toHaveBeenCalledTimes(1);
      expect(applyGraphDraftMutation).toHaveBeenCalledWith({
        graphPath,
        locale: "en-US",
        mutation: mutation(fixture),
      });
      expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toBe(before);
      expect(randomId).not.toHaveBeenCalled();

      pending.resolve({ status: "applied" });
      await expect(command).resolves.toBe(true);
      randomId.mockRestore();
    },
  );

  it.each([
    {
      type: "DuplicateSubgraph" as const,
      args: { nodeIds: ["node-a", "node-b"], offset: { x: 40, y: 40 } },
      mutation: {
        type: "duplicateSubgraph",
        payload: { nodeIds: ["node-a", "node-b"], offset: { x: 40, y: 40 } },
      },
    },
    {
      type: "InsertSubgraph" as const,
      args: { snapshotJson: '{"schemaVersion":1}', anchor: { x: 120, y: 240 } },
      mutation: {
        type: "insertSubgraph",
        payload: { snapshotJson: '{"schemaVersion":1}', anchor: { x: 120, y: 240 } },
      },
    },
  ])(
    "sends $type as one local draft edit and preserves its transformed result",
    async ({ type, args, mutation }) => {
      installProjection();
      const markGraphDirty = vi.spyOn(useExecutionStore.getState(), "markGraphDirty");
      const result = {
        projectInstanceId: "project-a",
        delta: {
          graphPath,
          fromRevision: 3,
          toRevision: 4,
          causedBy: "operation-a",
          payload: { operations: [] },
        },
        projectionReplacement: {} as never,
        history: { canUndo: true, canRedo: false },
      };
      applyGraphDraftMutation.mockResolvedValueOnce({ status: "applied", result });
      const randomId = vi.spyOn(crypto, "randomUUID");

      await expect(executeCommandWithResult(graphPath, type, args)).resolves.toEqual({
        status: "applied",
        result,
      });

      expect(applyGraphDraftMutation).toHaveBeenCalledOnce();
      expect(applyGraphDraftMutation).toHaveBeenCalledWith({
        graphPath,
        locale: "en-US",
        mutation,
      });
      expect(markGraphDirty).toHaveBeenCalledOnce();
      expect(markGraphDirty).toHaveBeenCalledWith(graphPath);
      expect(randomId).not.toHaveBeenCalled();
      randomId.mockRestore();
    },
  );

  it("sends InsertReroute without a disconnect, create, store write, or ID allocation", async () => {
    installProjection();
    const before = useGraphProjectionStore.getState().graphEntities[graphPath];
    const pending = deferred<{ status: "applied" }>();
    applyGraphDraftMutation.mockReturnValueOnce(pending.promise);
    const randomId = vi.spyOn(crypto, "randomUUID");

    const command = executeCommand(graphPath, "InsertReroute", {
      connectionId: "edge-1",
      position: { x: 120, y: 80 },
    });

    expect(applyGraphDraftMutation).toHaveBeenCalledTimes(1);
    expect(applyGraphDraftMutation).toHaveBeenCalledWith({
      graphPath,
      locale: "en-US",
      mutation: {
        type: "insertReroute",
        payload: { connectionId: "edge-1", position: { x: 120, y: 80 } },
      },
    });
    expect(
      applyGraphDraftMutation.mock.calls.flatMap(([input]) => input.mutation.type),
    ).not.toContain("disconnectConnections");
    expect(
      applyGraphDraftMutation.mock.calls.flatMap(([input]) => input.mutation.type),
    ).not.toContain("createNode");
    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toBe(before);
    expect(randomId).not.toHaveBeenCalled();

    pending.resolve({ status: "applied" });
    await expect(command).resolves.toBe(true);
    randomId.mockRestore();
  });

  it("keeps MoveNodes forward-only and sends final positions unchanged", async () => {
    installProjection();
    applyGraphDraftMutation.mockResolvedValueOnce({ status: "applied" });

    await expect(
      executeCommand(graphPath, "MoveNodes", {
        positions: [{ nodeId: "local-node", position: { x: 11, y: 29 } }],
      }),
    ).resolves.toBe(true);

    expect(applyGraphDraftMutation).toHaveBeenCalledTimes(1);
    expect(applyGraphDraftMutation).toHaveBeenCalledWith({
      graphPath,
      locale: "en-US",
      mutation: {
        type: "moveNodes",
        payload: {
          positions: [{ nodeId: "local-node", position: { x: 11, y: 29 } }],
        },
      },
    });
  });

  it.each([
    ["DeleteNodes", { nodeIds: [] }],
    ["DisconnectConnections", { connectionIds: [] }],
  ] as const)(
    "rejects empty direct arrays for %s before service invocation",
    async (type, args) => {
      installProjection();

      await expect(executeCommand(graphPath, type, args as never)).resolves.toBe(false);

      expect(applyGraphDraftMutation).not.toHaveBeenCalled();
    },
  );

  it("preserves a typed rejection for interaction callers without structural notification", async () => {
    const fixture = installProjection();
    const markGraphDirty = vi.spyOn(useExecutionStore.getState(), "markGraphDirty");
    const rejection = {
      status: "rejected" as const,
      code: "graph_connection_type_mismatch" as const,
    };
    applyGraphDraftMutation.mockResolvedValueOnce(rejection);

    await expect(
      executeCommandOutcome(graphPath, "ConnectPins", {
        pinA: fixture.inputKey,
        pinB: fixture.outputKey,
      }),
    ).resolves.toEqual(rejection);

    expect(applyGraphDraftMutation).toHaveBeenCalledTimes(1);
    expect(markGraphDirty).not.toHaveBeenCalled();
  });

  it.each([
    { status: "applied" as const, dirtyCalls: 1 },
    { status: "noop" as const, result: {} as never, dirtyCalls: 0 },
    { status: "rejected" as const, code: "graph_connection_type_mismatch" as const, dirtyCalls: 0 },
    { status: "saving" as const, dirtyCalls: 0 },
  ])("marks InsertReroute dirty only for $status", async (outcome) => {
    installProjection();
    const markGraphDirty = vi.spyOn(useExecutionStore.getState(), "markGraphDirty");
    applyGraphDraftMutation.mockResolvedValueOnce(outcome);

    await expect(
      executeCommandOutcome(graphPath, "InsertReroute", {
        connectionId: "edge-1",
        position: { x: 120, y: 80 },
      }),
    ).resolves.toEqual(outcome);

    expect(markGraphDirty).toHaveBeenCalledTimes(outcome.dirtyCalls);
    if (outcome.dirtyCalls === 1) expect(markGraphDirty).toHaveBeenCalledWith(graphPath);
  });

  it("preserves noop without structural notification", async () => {
    installProjection();
    const markGraphDirty = vi.spyOn(useExecutionStore.getState(), "markGraphDirty");
    const noop = { status: "noop" as const, result: {} as never };
    applyGraphDraftMutation.mockResolvedValueOnce(noop);

    await expect(
      executeCommandOutcome(graphPath, "DeleteNodes", {
        nodeIds: ["local-node"],
      }),
    ).resolves.toEqual(noop);

    expect(markGraphDirty).not.toHaveBeenCalled();
  });

  it("infers graph mutation command outcomes at runtime without erasing the discriminant", async () => {
    const fixture = installProjection();
    applyGraphDraftMutation.mockResolvedValueOnce({
      status: "rejected",
      code: "graph_connection_type_mismatch",
    });

    const outcome = await executeCommandOutcome(graphPath, "ConnectPins", {
      pinA: fixture.inputKey,
      pinB: fixture.outputKey,
    });

    if (outcome !== false && outcome.status === "rejected") {
      expect(outcome.code).toBe("graph_connection_type_mismatch");
    } else {
      expect.unreachable("unexpected non-rejected outcome");
    }
  });

  it("keeps the boolean command compatibility contract for rejected outcomes", async () => {
    const fixture = installProjection();
    applyGraphDraftMutation.mockResolvedValueOnce({
      status: "rejected",
      code: "graph_connection_type_mismatch",
    });

    await expect(
      executeCommand(graphPath, "ConnectPins", {
        pinA: fixture.inputKey,
        pinB: fixture.outputKey,
      }),
    ).resolves.toBe(false);
  });
});
