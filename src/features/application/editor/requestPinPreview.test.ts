import { beforeEach, describe, expect, it, vi } from "vitest";
import { portAddressKey } from "@/features/domain/editorProjection";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import * as projectLifecycleAuthority from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { markResourceLoaded, useDocumentStateStore } from "@/features/core/resource";
import { pinPreviewCacheKey, useExecutionStore } from "@/features/core/execution";
import { ProjectService } from "@/services/project/projectService";
import { PinPreviewGenerationService } from "@/services/nodeSystem/pinPreviewGenerationService";
import { normalizeIpcError } from "@/services/ipc";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import type { ExecutionDemandDto } from "@/shared/types/domain/executionDemand";
import type { RunEvent } from "@/shared/types/domain/runEvent";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import { requestPinPreview } from "./requestPinPreview";

const eventGraphPath = "events/Main.yssbi-event";
const frontendProjectInstanceId = "frontend-project-instance-1";
const backendProjectSessionId = "backend-project-session-1";

function runEvent(kind: RunEvent["kind"], runId = "run-1"): RunEvent {
  return {
    run: {
      projectSessionId: backendProjectSessionId,
      graphPath: eventGraphPath,
      runId,
    },
    kind,
  };
}

function installGraph(
  graphPath = eventGraphPath,
  outputAddress?: PortAddressDto,
): { outputKey: string; outputAddress: PortAddressDto; inputKey: string } {
  const fixture = makeEditorProjectionFixture({ graphPath });
  if (outputAddress) {
    fixture.projection.nodes[0].ports[0].address = outputAddress;
    fixture.projection.nodes[0].ports[0].templateKey =
      outputAddress.kind === "declared" ? outputAddress.portKey : outputAddress.templateKey;
    fixture.projection.connections[0].output = outputAddress;
  }
  const applied = useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
  expect(applied.applied).toBe(true);
  const kind = graphPath.startsWith("events/") ? "event" : "function";
  markResourceLoaded({ id: graphPath, kind });
  useGraphSessionStore.getState().setFocusedSession("editor-a", graphPath);
  return {
    outputKey: portAddressKey(outputAddress ?? fixture.outputAddress),
    outputAddress: outputAddress ?? fixture.outputAddress,
    inputKey: fixture.inputKey,
  };
}

function emitSuccessfulPreview(
  demand: ExecutionDemandDto,
  onEvent?: (event: RunEvent) => void,
  resultId = "result-1",
): void {
  if (demand.type !== "pinPreview") throw new Error("expected pin preview demand");
  onEvent?.(runEvent({ type: "runStarted" }));
  onEvent?.(
    runEvent({
      type: "pinPreviewResultReady",
      output: demand.output,
      generation: demand.generation,
      resultId,
    }),
  );
  onEvent?.(runEvent({ type: "runCompleted" }));
}

describe("requestPinPreview", () => {
  let nextGeneration = 0;

  beforeEach(() => {
    vi.restoreAllMocks();
    clearProjectLifecycle();
    startProjectLifecycle(frontendProjectInstanceId);
    useGraphDataStore.setState({ graphEntities: {} });
    useGraphSessionStore.getState().reset();
    useDocumentStateStore.getState().clear();
    useExecutionStore.setState({
      graphs: {},
      playbackGraphPath: null,
      isPlaying: false,
    });
    nextGeneration = 0;
    vi.spyOn(PinPreviewGenerationService, "allocate").mockImplementation(
      async () => ++nextGeneration,
    );
  });

  it.each([
    {
      name: "declared",
      address: {
        kind: "declared",
        nodeId: "local-node",
        portKey: "local-out",
      } as const,
    },
    {
      name: "dynamic instance",
      address: {
        kind: "instance",
        nodeId: "local-node",
        templateKey: "values",
        instanceId: "instance-7",
      } as const,
    },
  ])(
    "settles the exact projected $name preview when frontend and backend project identities differ",
    async ({ address }) => {
      const { outputKey } = installGraph(eventGraphPath, address);
      const execute = vi
        .spyOn(ProjectService, "executeGraphDocument")
        .mockImplementation(async (_projectInstanceId, _graphPath, demand, onEvent) => {
          emitSuccessfulPreview(demand, onEvent);
        });

      await expect(requestPinPreview(eventGraphPath, outputKey)).resolves.toMatchObject({
        status: "completed",
      });

      expect(execute).toHaveBeenCalledWith(
        frontendProjectInstanceId,
        eventGraphPath,
        {
          type: "pinPreview",
          output: { graphPath: eventGraphPath, port: address },
          generation: 1,
        },
        expect.any(Function),
      );
      expect(
        useExecutionStore
          .getState()
          .getGraph(eventGraphPath)
          .pinPreviews.get(pinPreviewCacheKey(eventGraphPath, address)),
      ).toMatchObject({ status: "ready", resultId: "result-1" });
    },
  );

  it("settles capture failure as a pure stale lifecycle rejection", async () => {
    const { outputKey } = installGraph();
    clearProjectLifecycle();
    const execute = vi.spyOn(ProjectService, "executeGraphDocument");
    const getExecutionState = vi.spyOn(useExecutionStore, "getState");

    await expect(requestPinPreview(eventGraphPath, outputKey)).resolves.toEqual({
      status: "rejected",
      reason: "stale-project-lifecycle",
    });

    expect(execute).not.toHaveBeenCalled();
    expect(getExecutionState).not.toHaveBeenCalled();
  });

  it("settles replacement before the pre-invoke assertion without side effects", async () => {
    const { outputKey } = installGraph();
    const capture = projectLifecycleAuthority.captureProjectIdentity;
    vi.spyOn(projectLifecycleAuthority, "captureProjectIdentity").mockImplementation(() => {
      const identity = capture();
      startProjectLifecycle("replacement-project-instance");
      return identity;
    });
    const execute = vi.spyOn(ProjectService, "executeGraphDocument");
    const getExecutionState = vi.spyOn(useExecutionStore, "getState");

    await expect(requestPinPreview(eventGraphPath, outputKey)).resolves.toEqual({
      status: "rejected",
      reason: "stale-project-lifecycle",
    });

    expect(execute).not.toHaveBeenCalled();
    expect(getExecutionState).not.toHaveBeenCalled();
  });

  it("rejects exhausted generation before any store write or IPC", async () => {
    const { outputKey } = installGraph();
    const before = useExecutionStore.getState();
    const beginPinPreview = vi.spyOn(before, "beginPinPreview");
    vi.mocked(PinPreviewGenerationService.allocate).mockRejectedValue(
      new Error("generation exhausted"),
    );
    const execute = vi.spyOn(ProjectService, "executeGraphDocument");

    await expect(requestPinPreview(eventGraphPath, outputKey)).resolves.toEqual({
      status: "rejected",
      reason: "generation-exhausted",
    });

    const after = useExecutionStore.getState();
    expect(after.graphs).toBe(before.graphs);
    expect(beginPinPreview).not.toHaveBeenCalled();
    expect(execute).not.toHaveBeenCalled();
  });

  it("returns a typed IPC failure without exposing backend details", async () => {
    const { outputKey, outputAddress } = installGraph();
    vi.spyOn(ProjectService, "executeGraphDocument").mockRejectedValue(
      normalizeIpcError("execute_graph_document", {
        code: "preview_execution_failed",
        details: { debug: "sensitive preview backend detail" },
        incidentId: "incident-preview-42",
      }),
    );

    const result = await requestPinPreview(eventGraphPath, outputKey);

    expect(result).toEqual({
      status: "failed",
      generation: 1,
      error: {
        code: "preview_execution_failed",
        incidentId: "incident-preview-42",
      },
    });
    const preview = useExecutionStore
      .getState()
      .getGraph(eventGraphPath)
      .pinPreviews.get(pinPreviewCacheKey(eventGraphPath, outputAddress));
    expect(preview).toMatchObject({ status: "error", error: "preview_execution_failed" });
    expect(JSON.stringify({ result, preview })).not.toContain("sensitive preview backend detail");
  });

  it.each([
    {
      name: "nested function graph",
      prepare: () => {
        const graphPath = "functions/Helper.yssbi-function";
        const graph = installGraph(graphPath);
        return { graphPath, pinId: graph.outputKey, reason: "nested-function" } as const;
      },
    },
    {
      name: "input pin",
      prepare: () => {
        const graph = installGraph();
        return { graphPath: eventGraphPath, pinId: graph.inputKey, reason: "input-pin" } as const;
      },
    },
    {
      name: "control output",
      prepare: () => {
        const graph = installGraph();
        useGraphDataStore.getState().graphEntities[eventGraphPath].pins[graph.outputKey].kind =
          "control";
        return {
          graphPath: eventGraphPath,
          pinId: graph.outputKey,
          reason: "non-data-output",
        } as const;
      },
    },
    {
      name: "effect output",
      prepare: () => {
        const graph = installGraph();
        useGraphDataStore.getState().graphEntities[eventGraphPath].pins[graph.outputKey].kind =
          "effect";
        return {
          graphPath: eventGraphPath,
          pinId: graph.outputKey,
          reason: "non-data-output",
        } as const;
      },
    },
    {
      name: "orphan output",
      prepare: () => {
        const graph = installGraph();
        useGraphDataStore.getState().graphEntities[eventGraphPath].pins[graph.outputKey].orphan =
          true;
        return { graphPath: eventGraphPath, pinId: graph.outputKey, reason: "orphan-pin" } as const;
      },
    },
    {
      name: "missing pin",
      prepare: () => {
        installGraph();
        return { graphPath: eventGraphPath, pinId: "missing-pin", reason: "missing-pin" } as const;
      },
    },
    {
      name: "missing projected address",
      prepare: () => {
        const graph = installGraph();
        delete useGraphDataStore.getState().graphEntities[eventGraphPath].pins[graph.outputKey]
          .address;
        return {
          graphPath: eventGraphPath,
          pinId: graph.outputKey,
          reason: "missing-address",
        } as const;
      },
    },
    {
      name: "missing focused graph session",
      prepare: () => {
        const graph = installGraph();
        useGraphSessionStore.getState().reset();
        return {
          graphPath: eventGraphPath,
          pinId: graph.outputKey,
          reason: "missing-session",
        } as const;
      },
    },
    {
      name: "unloaded graph resource",
      prepare: () => {
        const graph = installGraph();
        useDocumentStateStore.getState().clear();
        return {
          graphPath: eventGraphPath,
          pinId: graph.outputKey,
          reason: "missing-resource",
        } as const;
      },
    },
    {
      name: "missing graph projection",
      prepare: () => {
        useGraphSessionStore.getState().setFocusedSession("editor-a", eventGraphPath);
        markResourceLoaded({ id: eventGraphPath, kind: "event" });
        return { graphPath: eventGraphPath, pinId: "missing", reason: "missing-resource" } as const;
      },
    },
  ])("rejects $name before IPC", async ({ prepare }) => {
    const execute = vi.spyOn(ProjectService, "executeGraphDocument");
    const request = prepare();

    await expect(requestPinPreview(request.graphPath, request.pinId)).resolves.toEqual({
      status: "rejected",
      reason: request.reason,
    });

    expect(execute).not.toHaveBeenCalled();
  });

  it.each([
    {
      name: "projection object replacement",
      replace: () => {
        const current = useGraphDataStore.getState().graphEntities[eventGraphPath];
        useGraphDataStore.setState({
          graphEntities: {
            ...useGraphDataStore.getState().graphEntities,
            [eventGraphPath]: { ...current, pins: { ...current.pins } },
          },
        });
      },
    },
    {
      name: "request generation change",
      replace: () => {
        const current = useGraphDataStore.getState().graphEntities[eventGraphPath];
        current.requestGeneration += 1;
      },
    },
    {
      name: "source revision change",
      replace: () => {
        const current = useGraphDataStore.getState().graphEntities[eventGraphPath];
        current.sourceRevision += 1;
      },
    },
  ])("settles stale completion after $name as a pure no-op", async ({ replace }) => {
    const { outputKey } = installGraph();
    useExecutionStore.getState().startExecution(eventGraphPath);
    useExecutionStore.getState().setActiveRunId(eventGraphPath, "ordinary-run");
    const store = useExecutionStore.getState();
    const completePinPreview = vi.spyOn(store, "completePinPreview");
    const failPinPreview = vi.spyOn(store, "failPinPreview");
    const removePinPreview = vi.spyOn(store, "removePinPreview");
    vi.spyOn(ProjectService, "executeGraphDocument").mockImplementation(
      async (_projectInstanceId, _graphPath, demand, onEvent) => {
        if (demand.type !== "pinPreview") throw new Error("expected pin preview demand");
        onEvent?.(runEvent({ type: "runStarted" }));
        replace();
        onEvent?.(
          runEvent({
            type: "pinPreviewResultReady",
            output: demand.output,
            generation: demand.generation,
            resultId: "result-stale-projection",
          }),
        );
        onEvent?.(runEvent({ type: "runCompleted" }));
      },
    );

    const request = requestPinPreview(eventGraphPath, outputKey);
    await Promise.resolve();
    const previewSnapshot = structuredClone(store.getGraph(eventGraphPath));
    await expect(request).resolves.toEqual({
      status: "rejected",
      reason: "stale-project-lifecycle",
    });
    expect(store.getGraph(eventGraphPath)).toEqual(previewSnapshot);
    expect(completePinPreview).not.toHaveBeenCalled();
    expect(failPinPreview).not.toHaveBeenCalled();
    expect(removePinPreview).not.toHaveBeenCalled();
  });

  it.each(["resolution", "rejection"] as const)(
    "leaves a newer-generation replacement preview untouched after stale %s",
    async (settlement) => {
      const { outputKey, outputAddress } = installGraph();
      const originalStore = useExecutionStore.getState();
      const completePinPreview = vi.spyOn(originalStore, "completePinPreview");
      const failPinPreview = vi.spyOn(originalStore, "failPinPreview");
      const removePinPreview = vi.spyOn(originalStore, "removePinPreview");
      const setActiveRunId = vi.spyOn(originalStore, "setActiveRunId");
      let emit!: (event: RunEvent) => void;
      let resolveExecution!: () => void;
      let rejectExecution!: (reason: unknown) => void;
      const execute = vi.spyOn(ProjectService, "executeGraphDocument").mockImplementation(
        (_projectInstanceId, _graphPath, _demand, onEvent) =>
          new Promise((resolve, reject) => {
            emit = onEvent ?? (() => undefined);
            resolveExecution = resolve;
            rejectExecution = reject;
          }),
      );

      const preview = requestPinPreview(eventGraphPath, outputKey);
      await Promise.resolve();
      const cacheKey = pinPreviewCacheKey(eventGraphPath, outputAddress);
      const originalGeneration = originalStore
        .getGraph(eventGraphPath)
        .pinPreviews.get(cacheKey)?.generation;
      expect(execute).toHaveBeenCalledWith(
        frontendProjectInstanceId,
        eventGraphPath,
        expect.objectContaining({ type: "pinPreview" }),
        expect.any(Function),
      );

      startProjectLifecycle("project-session-2");
      useExecutionStore.setState({
        graphs: {},
        playbackGraphPath: null,
        isPlaying: false,
      });
      const replacementStore = useExecutionStore.getState();
      replacementStore.startExecution(eventGraphPath);
      replacementStore.setActiveRunId(eventGraphPath, "replacement-run");
      const replacementLease = replacementStore.beginPinPreview(
        eventGraphPath,
        outputAddress,
        (originalGeneration ?? 0) + 1,
      );
      const replacementGeneration = replacementLease.generation;
      expect(replacementGeneration).not.toBe(originalGeneration);
      const replacementGraphSnapshot = structuredClone(replacementStore.getGraph(eventGraphPath));
      completePinPreview.mockClear();
      failPinPreview.mockClear();
      removePinPreview.mockClear();
      setActiveRunId.mockClear();
      const getExecutionState = vi.spyOn(useExecutionStore, "getState");

      if (settlement === "resolution") {
        emit(runEvent({ type: "runStarted" }));
        emit(
          runEvent({
            type: "pinPreviewResultReady",
            output: { graphPath: eventGraphPath, port: outputAddress },
            generation: originalGeneration ?? 0,
            resultId: "result-stale-project",
          }),
        );
        emit(runEvent({ type: "runCompleted" }));
        resolveExecution();
      } else {
        rejectExecution({ code: "stale_request", message: "old request rejected" });
      }

      await expect(preview).resolves.toEqual({
        status: "rejected",
        reason: "stale-project-lifecycle",
      });
      expect(getExecutionState).not.toHaveBeenCalled();
      expect(completePinPreview).not.toHaveBeenCalled();
      expect(failPinPreview).not.toHaveBeenCalled();
      expect(removePinPreview).not.toHaveBeenCalled();
      expect(setActiveRunId).not.toHaveBeenCalled();
      expect(replacementStore.getGraph(eventGraphPath)).toEqual(replacementGraphSnapshot);
    },
  );

  it("settles rejection after projection replacement as a pure no-op", async () => {
    const { outputKey } = installGraph();
    useExecutionStore.getState().startExecution(eventGraphPath);
    useExecutionStore.getState().setActiveRunId(eventGraphPath, "ordinary-run");
    const commandError = { code: "test_stop", message: "stale after invoke" };
    const store = useExecutionStore.getState();
    const failPinPreview = vi.spyOn(store, "failPinPreview");
    const removePinPreview = vi.spyOn(store, "removePinPreview");
    vi.spyOn(ProjectService, "executeGraphDocument").mockImplementation(async () => {
      const current = useGraphDataStore.getState().graphEntities[eventGraphPath];
      useGraphDataStore.setState({
        graphEntities: {
          ...useGraphDataStore.getState().graphEntities,
          [eventGraphPath]: { ...current, pins: { ...current.pins } },
        },
      });
      throw commandError;
    });

    const request = requestPinPreview(eventGraphPath, outputKey);
    await Promise.resolve();
    const previewSnapshot = structuredClone(store.getGraph(eventGraphPath));
    await expect(request).resolves.toEqual({
      status: "rejected",
      reason: "stale-project-lifecycle",
    });
    expect(store.getGraph(eventGraphPath)).toEqual(previewSnapshot);
    expect(failPinPreview).not.toHaveBeenCalled();
    expect(removePinPreview).not.toHaveBeenCalled();
  });

  it("does not let stale cleanup remove a newer preview generation for the same pin", async () => {
    const { outputKey, outputAddress } = installGraph();
    const pending: Array<{
      reject: (reason: unknown) => void;
    }> = [];
    vi.spyOn(ProjectService, "executeGraphDocument").mockImplementation(
      () => new Promise((_resolve, reject) => pending.push({ reject })),
    );

    const staleRequest = requestPinPreview(eventGraphPath, outputKey);
    await Promise.resolve();
    const previous = useGraphDataStore.getState().graphEntities[eventGraphPath];
    useGraphDataStore.setState({
      graphEntities: {
        ...useGraphDataStore.getState().graphEntities,
        [eventGraphPath]: { ...previous, pins: { ...previous.pins } },
      },
    });
    const currentRequest = requestPinPreview(eventGraphPath, outputKey);
    await Promise.resolve();
    const currentBeforeCleanup = useExecutionStore
      .getState()
      .getGraph(eventGraphPath)
      .pinPreviews.get(pinPreviewCacheKey(eventGraphPath, outputAddress));
    if (!currentBeforeCleanup) throw new Error("expected newer pending preview");

    pending[0].reject({ code: "stale_request", message: "old request rejected" });
    await expect(staleRequest).resolves.toMatchObject({ status: "rejected" });

    expect(
      useExecutionStore
        .getState()
        .getGraph(eventGraphPath)
        .pinPreviews.get(pinPreviewCacheKey(eventGraphPath, outputAddress)),
    ).toMatchObject({
      generation: currentBeforeCleanup.generation,
      status: "pending",
    });

    pending[1].reject({ code: "test_stop", message: "finish current request" });
    await currentRequest;
  });

  it("suppresses the older completion when two previews race", async () => {
    const { outputKey, outputAddress } = installGraph();
    const callbacks: Array<(event: RunEvent) => void> = [];
    const resolvers: Array<() => void> = [];
    vi.spyOn(ProjectService, "executeGraphDocument").mockImplementation(
      (_projectInstanceId, _graphPath, _demand, onEvent) =>
        new Promise((resolve) => {
          callbacks.push(onEvent ?? (() => undefined));
          resolvers.push(resolve);
        }),
    );

    const first = requestPinPreview(eventGraphPath, outputKey);
    const second = requestPinPreview(eventGraphPath, outputKey);
    await Promise.resolve();
    const store = useExecutionStore.getState();
    const completePinPreview = vi.spyOn(store, "completePinPreview");
    const failPinPreview = vi.spyOn(store, "failPinPreview");
    const removePinPreview = vi.spyOn(store, "removePinPreview");
    const getExecutionState = vi.spyOn(useExecutionStore, "getState");
    completePinPreview.mockClear();
    failPinPreview.mockClear();
    removePinPreview.mockClear();

    callbacks[0](runEvent({ type: "runStarted" }, "run-old"));
    callbacks[0](
      runEvent(
        {
          type: "pinPreviewResultReady",
          output: { graphPath: eventGraphPath, port: outputAddress },
          generation: 1,
          resultId: "result-old",
        },
        "run-old",
      ),
    );
    callbacks[0](runEvent({ type: "runCompleted" }, "run-old"));
    resolvers[0]();
    await expect(first).resolves.toEqual({
      status: "rejected",
      reason: "stale-project-lifecycle",
    });

    expect(getExecutionState).not.toHaveBeenCalled();
    expect(completePinPreview).not.toHaveBeenCalled();
    expect(failPinPreview).not.toHaveBeenCalled();
    expect(removePinPreview).not.toHaveBeenCalled();

    callbacks[1](runEvent({ type: "runStarted" }, "run-new"));
    callbacks[1](
      runEvent(
        {
          type: "pinPreviewResultReady",
          output: { graphPath: eventGraphPath, port: outputAddress },
          generation: 2,
          resultId: "result-new",
        },
        "run-new",
      ),
    );
    callbacks[1](runEvent({ type: "runCompleted" }, "run-new"));
    resolvers[1]();
    await second;

    expect(
      useExecutionStore
        .getState()
        .getGraph(eventGraphPath)
        .pinPreviews.get(pinPreviewCacheKey(eventGraphPath, outputAddress)),
    ).toMatchObject({ status: "ready", resultId: "result-new" });
  });

  it("settles a stale wire generation as a pure no-op", async () => {
    const { outputKey } = installGraph();
    vi.spyOn(ProjectService, "executeGraphDocument").mockImplementation(
      async (_projectInstanceId, _graphPath, demand, onEvent) => {
        if (demand.type !== "pinPreview") throw new Error("expected pin preview demand");
        onEvent?.(runEvent({ type: "runStarted" }));
        onEvent?.(
          runEvent({
            type: "pinPreviewResultReady",
            output: demand.output,
            generation: demand.generation + 1,
            resultId: "result-stale-generation",
          }),
        );
        onEvent?.(runEvent({ type: "runCompleted" }));
      },
    );

    const store = useExecutionStore.getState();
    const failPinPreview = vi.spyOn(store, "failPinPreview");
    const removePinPreview = vi.spyOn(store, "removePinPreview");
    failPinPreview.mockClear();
    removePinPreview.mockClear();
    const request = requestPinPreview(eventGraphPath, outputKey);
    await Promise.resolve();
    const previewSnapshot = structuredClone(store.getGraph(eventGraphPath));

    await expect(request).resolves.toEqual({
      status: "rejected",
      reason: "stale-project-lifecycle",
    });
    expect(store.getGraph(eventGraphPath)).toEqual(previewSnapshot);
    expect(failPinPreview).not.toHaveBeenCalled();
    expect(removePinPreview).not.toHaveBeenCalled();
  });
});
