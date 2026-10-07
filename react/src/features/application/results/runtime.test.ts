import { installGraphRunEvent } from "@/features/application/editor/observeGraphRunEvent";
import {
  makeGraphEditorSession,
  makeEditorProjectionFixture,
} from "@/tests/helpers/editorProjectionFixtures";
import { resultSessionFixture, resultReferenceFixture } from "@/tests/helpers/resultFixture";
import * as invalidationChannel from "@/services/result/resultSessionChannel";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GraphResultState, ResultDescriptor, ResultPage } from "@/shared/types/domain/result";
import type { RunEventKind } from "@/shared/types/domain/runEvent";
import { ResultService } from "@/services/result/resultService";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { useExecutionStore } from "@/features/core/execution";
import * as graphPresentation from "./graphPresentation";
import * as readProjection from "@/features/core/state/readProjection";

import {
  prepareResultExecutionSession,
  resetResultQueryProject,
  resultQueryCoordinator,
  resultQueryRead,
  reconcileGraphResultQueries,
  resetGraphResultQueries,
  usePinResultSearchEntries,
} from "./runtime";

const graphPath = "events/Main.yssbi-event";
const fixture = makeEditorProjectionFixture({ graphPath });
const output = { graphPath, port: fixture.outputAddress };
const request = { graphPath, output: output.port };
function resultState(
  resultId: string | null,
  executionSessionId = resultSessionFixture,
): GraphResultState {
  return {
    revision: "0",

    executionSessionId,
    semanticInputHash:
      useResourceStore.getState().sessions[graphPath]?.semanticInputHash ??
      fixture.projection.basis.semanticInputHash,
    outputs: [{ output, state: resultId ? "valid" : "missing", resultId }],
    connections: [],
  };
}
const descriptor = (id: string): ResultDescriptor => ({
  resultId: id,

  executionSessionId: resultSessionFixture,
  provenance: {
    runId: id,
    createdAtMs: "1000",
    graphPath,
    nodeId: output.port.nodeId,
    output,
  },
  presentation: { kind: "inspector" },
  valueKind: "sequence",
  totalCount: 1,
  metadata: null,
  title: "Result",
});
const page = (id: string): ResultPage => ({
  resultId: id,
  offset: 0,
  requestedLimit: 200,
  actualCount: 1,
  totalCount: 1,
  hasMore: false,
  nextOffset: null,
  valueKind: "sequence",
  metadata: null,
  values: [[1]],
});
function event(runId: string, kind: RunEventKind, resultRevision = runId) {
  installGraphRunEvent({
    resultRevision,
    run: {
      runId,
      graphPath,
      executionSessionId: resultSessionFixture,
      semanticInputHash: "0".repeat(64),
    },
    kind,
  });
}

beforeEach(() => {
  vi.restoreAllMocks();
  resetResultQueryProject();
  useExecutionStore.setState({ graphs: {} });
  useResourceStore.getState().clear();
  startProjectLifecycle("project-1");
  useResourceStore.getState().installGraphSession(
    graphPath,
    {
      ...makeGraphEditorSession(fixture.projection),
      resultState: resultState("1"),
    },
    { mode: "load" },
  );
  vi.spyOn(ResultService, "getGraphState").mockResolvedValue(null);
});

describe("current result lifecycle", () => {
  it.each(["nullPin", "runInvalidation"])(
    "preserves successor graph queries after %s notifications",
    async (action) => {
      const pinRead = vi.spyOn(ResultService, "getPinResult");
      for (const replacement of ["project", "frame", "summary"]) {
        startProjectLifecycle("project-1");
        resetResultQueryProject();
        useExecutionStore.setState({ graphs: {} });
        prepareResultExecutionSession(resultSessionFixture);
        useResourceStore.getState().installGraphSession(
          graphPath,
          {
            ...makeGraphEditorSession(fixture.projection),
            resultState: resultState("1"),
          },
          { mode: "load" },
        );
        pinRead.mockResolvedValue(descriptor("1"));
        await resultQueryCoordinator.loadPinResult(request);
        pinRead.mockResolvedValue(null);
        const settles: Array<(value: GraphResultState | null) => void> = [];
        vi.mocked(ResultService.getGraphState)
          .mockReset()
          .mockImplementation(
            () =>
              new Promise((resolve) => {
                settles.push(resolve);
              }),
          );
        let successorQuery: ReturnType<typeof resultQueryCoordinator.loadGraphState> | undefined;
        let replaced = false;
        const stop = resultQueryRead.subscribe(() => {
          if (replaced) return;
          replaced = true;
          if (replacement === "project") {
            startProjectLifecycle("project-1");
            resetResultQueryProject();
            prepareResultExecutionSession(resultSessionFixture);
          } else if (replacement === "frame") {
            useResourceStore.getState().removeGraphSession(graphPath);
            useResourceStore.getState().installGraphSession(
              graphPath,
              {
                ...makeGraphEditorSession(fixture.projection),
                resultState: resultState("2"),
              },
              { mode: "load" },
            );
          } else {
            useResourceStore.getState().setGraphResultState(graphPath, {
              ...resultState("2"),
              revision: "1",
            });
          }
          const session = useResourceStore.getState().sessions[graphPath];
          successorQuery = resultQueryCoordinator.loadGraphState({
            graphPath,
            sessionId: session.sessionId,
            projectionGeneration: session.projectionGeneration,
            semanticInputHash: session.semanticInputHash,
          });
        });
        try {
          if (action === "nullPin") await resultQueryCoordinator.loadPinResult(request);
          else event("2", { type: "runStarted", outputs: [output] });
          await Promise.resolve();
          expect.soft(replaced, replacement).toBe(true);
          expect.soft(ResultService.getGraphState, replacement).toHaveBeenCalledTimes(1);
        } finally {
          stop();
        }
        settles.forEach((resolve) => resolve(null));
        await expect.soft(successorQuery, replacement).resolves.toEqual({ status: "notReady" });
      }
    },
  );

  it("preserves a successor query when graph state publication replaces its frame", async () => {
    const pinRead = vi.spyOn(ResultService, "getPinResult");
    vi.spyOn(ResultService, "getValue").mockResolvedValue({ kind: "value", value: 42 });
    for (const boundary of ["sessionReset", "summaryPublication", "newerSummary"]) {
      resetResultQueryProject();
      prepareResultExecutionSession(resultSessionFixture);
      await resultQueryCoordinator.loadValue(resultReferenceFixture("1"));
      const incomingSession =
        boundary === "sessionReset" ? "00000000-0000-0000-0000-000000000002" : resultSessionFixture;
      vi.mocked(ResultService.getGraphState).mockResolvedValue(resultState("2", incomingSession));
      const settles: Array<(value: ResultDescriptor) => void> = [];
      pinRead.mockReset().mockImplementation(
        () =>
          new Promise((resolve) => {
            settles.push(resolve);
          }),
      );
      let successorQuery: ReturnType<typeof resultQueryCoordinator.loadPinResult> | undefined;
      let successorState: ReturnType<typeof useResourceStore.getState>["resultStates"][string];
      let replaced = false;
      const replace = () => {
        if (replaced) return;
        replaced = true;
        if (boundary === "newerSummary") {
          useResourceStore.getState().setGraphResultState(graphPath, {
            ...resultState("3", incomingSession),
            revision: "1",
          });
        } else {
          useResourceStore.getState().removeGraphSession(graphPath);
          useResourceStore.getState().installGraphSession(
            graphPath,
            {
              ...makeGraphEditorSession(fixture.projection),
              resultState: resultState("3", incomingSession),
            },
            { mode: "load" },
          );
        }
        successorState = useResourceStore.getState().resultStates[graphPath];
        successorQuery = resultQueryCoordinator.loadPinResult(request);
      };
      const stop =
        boundary === "sessionReset"
          ? resultQueryRead.subscribe(replace)
          : useResourceStore.subscribe(replace);
      const session = useResourceStore.getState().sessions[graphPath];
      try {
        await resultQueryCoordinator.loadGraphState({
          graphPath,
          sessionId: session.sessionId,
          projectionGeneration: session.projectionGeneration,
          semanticInputHash: session.semanticInputHash,
        });
        expect.soft(replaced, boundary).toBe(true);
        expect
          .soft(useResourceStore.getState().resultStates[graphPath], boundary)
          .toBe(successorState!);
        expect.soft(pinRead, boundary).toHaveBeenCalledTimes(1);
      } finally {
        stop();
      }
      settles.forEach((resolve) =>
        resolve({ ...descriptor("3"), executionSessionId: incomingSession }),
      );
      await expect.soft(successorQuery, boundary).resolves.toEqual({ status: "published" });
      expect.soft(resultQueryRead.getPinResult(request)?.resultId, boundary).toBe("3");
    }
  });

  it("does not supersede new pin reads during graph reconciliation notifications", async () => {
    const pinRead = vi.spyOn(ResultService, "getPinResult");
    vi.spyOn(ResultService, "getValue").mockResolvedValue({ kind: "value", value: 42 });
    for (const boundary of ["executionRelease", "pinInvalidation"]) {
      resetResultQueryProject();
      prepareResultExecutionSession(resultSessionFixture);
      useResourceStore.getState().installGraphSession(
        graphPath,
        {
          ...makeGraphEditorSession(fixture.projection),
          resultState: {
            ...resultState("1"),
            semanticInputHash: fixture.projection.basis.semanticInputHash,
          },
        },
        { mode: "load" },
      );
      pinRead.mockReset().mockResolvedValue(descriptor("1"));
      await resultQueryCoordinator.loadPinResult(request);
      await resultQueryCoordinator.loadValue(resultReferenceFixture("1"));
      const previous = useResourceStore.getState().sessions[graphPath];
      const changed = structuredClone(fixture.projection);
      changed.basis.semanticInputHash = "1".repeat(64);
      useResourceStore.getState().installGraphSession(graphPath, {
        ...makeGraphEditorSession(changed),
        resultState: { ...resultState("2"), semanticInputHash: changed.basis.semanticInputHash },
      });
      useExecutionStore.getState().submitExecution(graphPath);
      const settles: Array<(value: ResultDescriptor) => void> = [];
      pinRead.mockReset().mockImplementation(
        () =>
          new Promise((resolve) => {
            settles.push(resolve);
          }),
      );
      let successorQuery: ReturnType<typeof resultQueryCoordinator.loadPinResult> | undefined;
      let replaced = false;
      const replace = () => {
        if (replaced) return;
        replaced = true;
        startProjectLifecycle("project-1");
        resetResultQueryProject();
        prepareResultExecutionSession(resultSessionFixture);
        useResourceStore.getState().installGraphSession(
          graphPath,
          {
            ...makeGraphEditorSession(fixture.projection),
            resultState: {
              ...resultState("3"),
              semanticInputHash: fixture.projection.basis.semanticInputHash,
            },
          },
          { mode: "load" },
        );
        successorQuery = resultQueryCoordinator.loadPinResult(request);
      };
      const stop =
        boundary === "executionRelease"
          ? useExecutionStore.subscribe(replace)
          : resultQueryRead.subscribe(replace);
      try {
        reconcileGraphResultQueries(graphPath, previous);
        expect.soft(replaced, boundary).toBe(true);
        expect.soft(pinRead, boundary).toHaveBeenCalledTimes(1);
      } finally {
        stop();
      }
      settles.forEach((resolve) => resolve(descriptor("3")));
      await expect.soft(successorQuery, boundary).resolves.toEqual({ status: "published" });
      expect(resultQueryRead.getPinResult(request)?.resultId).toBe("3");
    }
  });

  it("preserves successor output runs published during a graph query reset", async () => {
    vi.spyOn(ResultService, "getPinResult").mockResolvedValue(descriptor("1"));
    for (const replaceProject of [false, true]) {
      useExecutionStore.setState({ graphs: {} });
      useResourceStore.getState().setGraphResultState(graphPath, resultState("1"));
      await resultQueryCoordinator.loadPinResult(request);
      let replaced = false;
      let successorRuns: ReturnType<
        typeof useExecutionStore.getState
      >["graphs"][string]["outputRuns"];
      const stop = resultQueryRead.subscribe(() => {
        if (replaced) return;
        replaced = true;
        if (replaceProject) startProjectLifecycle("successor");
        event("2", { type: "runStarted", outputs: [output] });
        successorRuns = useExecutionStore.getState().graphs[graphPath].outputRuns;
      });
      try {
        resetGraphResultQueries(graphPath);
        expect.soft(replaced).toBe(true);
        expect.soft(Object.values(successorRuns!)).toHaveLength(1);
        expect.soft(useExecutionStore.getState().graphs[graphPath].outputRuns).toBe(successorRuns!);
      } finally {
        stop();
      }
    }
  });

  it("preserves successor runs and execution sessions during a synchronous reset", async () => {
    const publishInvalidation = vi.spyOn(invalidationChannel, "publishResultSessionEnd");
    const reference = resultReferenceFixture("1");
    vi.spyOn(ResultService, "getValue").mockResolvedValue({ kind: "value", value: 42 });
    for (const replaceProject of [true, false]) {
      prepareResultExecutionSession(resultSessionFixture);
      await resultQueryCoordinator.loadValue(reference);
      const successorSession = replaceProject
        ? "00000000-0000-0000-0000-000000000002"
        : "00000000-0000-0000-0000-000000000003";
      let successorRuns: ReturnType<
        typeof useExecutionStore.getState
      >["graphs"][string]["outputRuns"];
      let replaced = false;
      const stop = resultQueryRead.subscribe(() => {
        if (replaced) return;
        replaced = true;
        if (replaceProject) startProjectLifecycle("project-2");
        prepareResultExecutionSession(successorSession);
        useResourceStore.getState().installGraphSession(graphPath, {
          ...makeGraphEditorSession(fixture.projection),
          resultState: resultState(null, successorSession),
        });
        installGraphRunEvent({
          resultRevision: "2",
          run: {
            runId: "2",
            graphPath,
            executionSessionId: successorSession,
            semanticInputHash: fixture.projection.basis.semanticInputHash,
          },
          kind: { type: "runStarted", outputs: [output] },
        });
        successorRuns = useExecutionStore.getState().graphs[graphPath].outputRuns;
      });
      try {
        const prepared = prepareResultExecutionSession("00000000-0000-0000-0000-000000000004");
        expect(replaced).toBe(true);
        expect(useExecutionStore.getState().graphs[graphPath].outputRuns).toBe(successorRuns!);
        expect(Object.values(successorRuns!)).toHaveLength(1);
        expect(prepared).toBe(false);
      } finally {
        stop();
      }
      resetResultQueryProject();
      expect(publishInvalidation).toHaveBeenLastCalledWith(successorSession);
    }
  });

  it("confirms run invalidation only from a summary covering the backend result revision", async () => {
    const compose = vi.spyOn(graphPresentation, "projectGraphPresentation");
    const latestCache = () => {
      const result = compose.mock.results[compose.mock.results.length - 1];
      if (result?.type !== "return") throw new Error("Graph presentation was not composed");
      return result.value.nodes[output.port.nodeId].cache;
    };
    event("2", { type: "runStarted", outputs: [output] }, "9007199254740992");
    event("2", { type: "runCompleted" }, "9007199254740993");
    const session = useResourceStore.getState().sessions[graphPath];
    vi.mocked(ResultService.getGraphState).mockResolvedValue({
      ...resultState("1"),
      revision: "9007199254740992",
    });
    await resultQueryCoordinator.loadGraphState({
      graphPath,
      sessionId: session.sessionId,
      projectionGeneration: session.projectionGeneration,
      semanticInputHash: session.semanticInputHash,
    });
    expect.soft(latestCache()).toBe("new");

    compose.mockClear();
    const execution = useExecutionStore.getState().graphs[graphPath];
    useResourceStore.getState().installGraphSession(graphPath, {
      ...makeGraphEditorSession(fixture.projection),
      resultState: { ...resultState("2"), revision: "9007199254740993" },
    });
    expect(latestCache()).toBe("valid");
    expect(compose).toHaveBeenCalledTimes(1);
    expect(useExecutionStore.getState().graphs[graphPath]).toBe(execution);
  });

  it("hides invalid current pins as soon as a graph frame changes while retaining held data", async () => {
    vi.spyOn(ResultService, "getPinResult").mockResolvedValue(descriptor("1"));
    vi.spyOn(ResultService, "getValue").mockResolvedValue({ kind: "value", value: 42 });
    await resultQueryCoordinator.loadPinResult(request);
    const reference = resultReferenceFixture("1");
    const release = resultQueryCoordinator.retainPayload(reference);
    await resultQueryCoordinator.loadValue(reference);
    const held = resultQueryRead.getValue(reference);
    const seenByGraph: unknown[] = [];
    const seenByResults: unknown[] = [];
    const stopGraph = useResourceStore.subscribe(() =>
      seenByGraph.push(resultQueryRead.getPinResult(request)),
    );
    const stopResults = resultQueryRead.subscribe(() =>
      seenByResults.push(resultQueryRead.getPinResult(request)),
    );
    try {
      const changed = structuredClone(fixture.projection);
      changed.basis.semanticInputHash = "2".repeat(64);
      const previous = useResourceStore.getState().sessions[graphPath];
      useResourceStore.getState().installGraphSession(graphPath, {
        ...makeGraphEditorSession(changed),
        resultState: { ...resultState(null), semanticInputHash: changed.basis.semanticInputHash },
      });
      expect.soft(seenByGraph).toEqual([null]);
      expect.soft(seenByResults).toEqual([null]);
      expect.soft(resultQueryRead.getPinResult(request)).toBeNull();
      reconcileGraphResultQueries(graphPath, previous);
      expect(resultQueryRead.getValue(reference)).toBe(held);
      expect(resultQueryRead.getDescriptor(reference)?.resultId).toBe("1");
      expect(seenByResults).toEqual([null]);
    } finally {
      stopGraph();
      stopResults();
      release();
    }
  });

  it("keeps valid pin bindings stable until the authoritative result identity changes", async () => {
    vi.spyOn(ResultService, "getPinResult").mockResolvedValue(descriptor("1"));
    await resultQueryCoordinator.loadPinResult(request);
    const first = resultQueryRead.getPinResult(request);
    const observed: (string | null)[] = [];
    const stop = resultQueryRead.subscribe(() =>
      observed.push(resultQueryRead.getPinResult(request)?.resultId ?? null),
    );
    try {
      const state = useResourceStore.getState().resultStates[graphPath];
      useResourceStore.getState().setGraphResultState(graphPath, { ...state, revision: "1" });
      expect(observed).toEqual([]);
      expect(resultQueryRead.getPinResult(request)).toBe(first);
      useResourceStore
        .getState()
        .setGraphResultState(graphPath, { ...resultState("2"), revision: "2" });
      expect.soft(observed).toEqual([null]);
      expect.soft(resultQueryRead.getPinResult(request)).toBeNull();
      vi.mocked(ResultService.getPinResult).mockResolvedValue(descriptor("2"));
      await resultQueryCoordinator.loadPinResult(request);
      expect(observed).toEqual([null, "2"]);
    } finally {
      stop();
    }
  });

  it("keeps search reads graph-scoped and removes invalid bindings in the same publication", async () => {
    // Read the real application projection at its React subscription boundary; no UI is mounted.
    vi.spyOn(readProjection, "useReadProjection").mockImplementation((projection, select) =>
      select(projection.getSnapshot()),
    );
    vi.spyOn(ResultService, "getPinResult").mockResolvedValue(descriptor("1"));
    await resultQueryCoordinator.loadPinResult(request);
    const first = usePinResultSearchEntries(graphPath);
    expect(first).toHaveLength(1);
    expect(first[0].nodeTitle).toBe("Projected node");

    const otherPath = "events/Other.yssbi-event";
    const otherOutput = { ...output, graphPath: otherPath };
    const other = descriptor("2");
    other.provenance = { ...other.provenance, graphPath: otherPath, output: otherOutput };
    vi.mocked(ResultService.getPinResult).mockResolvedValue(other);
    await resultQueryCoordinator.loadPinResult({ graphPath: otherPath, output: otherOutput.port });
    const otherEntries = usePinResultSearchEntries(otherPath);
    expect(otherEntries).toHaveLength(1);
    expect(usePinResultSearchEntries(graphPath)).toBe(first);

    const observed: ReturnType<typeof usePinResultSearchEntries>[] = [];
    const stop = resultQueryRead.subscribe(() =>
      observed.push(usePinResultSearchEntries(graphPath)),
    );
    try {
      useResourceStore.getState().setGraphResultState(graphPath, {
        ...resultState(null),
        revision: "1",
      });
      expect(observed).toEqual([[]]);
      expect(usePinResultSearchEntries(graphPath)).toHaveLength(0);
      expect(usePinResultSearchEntries(otherPath)).toBe(otherEntries);
      expect(first).toHaveLength(1);
    } finally {
      stop();
    }
  });

  it.each(["runStarted", "graphReset"])(
    "publishes all invalidated pins once during %s while retaining held payloads",
    async (action) => {
      useResourceStore.getState().removeGraphSession(graphPath);
      const secondOutput = {
        graphPath,
        port: { kind: "declared" as const, nodeId: output.port.nodeId, portKey: "second" },
      };
      const secondRequest = { graphPath, output: secondOutput.port };
      const second = descriptor("2");
      second.provenance.output = secondOutput;
      vi.spyOn(ResultService, "getPinResult").mockImplementation(async (_graph, port) =>
        port.kind === "declared" && port.portKey === "second" ? second : descriptor("1"),
      );
      vi.spyOn(ResultService, "getValue").mockResolvedValue({ kind: "value", value: 42 });
      await resultQueryCoordinator.loadPinResult(request);
      await resultQueryCoordinator.loadPinResult(secondRequest);
      await resultQueryCoordinator.loadValue(resultReferenceFixture("1"));
      await resultQueryCoordinator.loadValue(resultReferenceFixture("2"));
      const release = resultQueryCoordinator.retainPayload(resultReferenceFixture("2"));
      const heldValue = resultQueryRead.getValue(resultReferenceFixture("2"));
      const observed: (string | null)[][] = [];
      const unsubscribe = resultQueryRead.subscribe(() => {
        observed.push(
          [request, secondRequest].map(
            (pin) => resultQueryRead.getPinResult(pin)?.resultId ?? null,
          ),
        );
      });

      if (action === "runStarted")
        event("3", { type: "runStarted", outputs: [output, secondOutput] });
      else resetGraphResultQueries(graphPath);
      unsubscribe();

      expect(observed).toEqual([[null, null]]);
      expect(resultQueryRead.getValue(resultReferenceFixture("1"))).toBeNull();
      expect(resultQueryRead.getValue(resultReferenceFixture("2"))).toBe(heldValue);
      expect(resultQueryRead.getDescriptor(resultReferenceFixture("2"))?.resultId).toBe("2");
      release();
    },
  );

  it("rebuilds result tables only for changed result branches or pending invalidations", async () => {
    const aggregate = vi.spyOn(graphPresentation, "projectGraphResultCache");
    const compose = vi.spyOn(graphPresentation, "projectGraphPresentation");
    const latest = () => {
      const result = compose.mock.results[compose.mock.results.length - 1];
      if (result?.type !== "return") throw new Error("Graph presentation was not composed");
      return result.value;
    };
    const store = useResourceStore.getState();
    const current = store.resultStates[graphPath];
    const stale: GraphResultState = {
      ...current,
      revision: "1",
      outputs: current.outputs.map((entry) => ({
        ...entry,
        state: "stale",
        resultId: entry.resultId ?? "1",
      })),
    };
    store.setGraphResultState(graphPath, stale);
    expect(aggregate).toHaveBeenCalledTimes(1);
    const initial = latest();
    expect(initial.nodes[output.port.nodeId].cache).toBe("stale");
    aggregate.mockClear();

    const execution = useExecutionStore.getState();
    event("1", { type: "runStarted", outputs: [] });
    event("1", {
      type: "runErrored",
      groups: [],
      code: "kernelFailed",
      phase: "execution",
      source: { graphPath, nodeId: output.port.nodeId, portAddress: null },
    });
    const failed = latest();
    expect(failed.failure?.code).toBe("kernelFailed");
    expect(failed.nodes).toBe(initial.nodes);
    expect(failed.outputs).toBe(initial.outputs);
    expect(failed.connections).toBe(initial.connections);
    const changed = structuredClone(fixture.projection);
    changed.basis.semanticInputHash = "2".repeat(64);
    store.installGraphSession(graphPath, {
      ...makeGraphEditorSession(changed),
      resultState: { ...stale, semanticInputHash: changed.basis.semanticInputHash },
    });
    expect(latest().failure).toBeNull();
    expect(useExecutionStore.getState().getGraph(graphPath).runFailure).toBeTruthy();
    store.installGraphSession(graphPath, {
      ...makeGraphEditorSession(fixture.projection),
      resultState: stale,
    });
    aggregate.mockClear();
    execution.clearRunFailure(graphPath);
    store.setGraphResultState(graphPath, { ...stale, revision: "2" });
    expect(aggregate).not.toHaveBeenCalled();
    expect(latest().failure).toBeNull();

    event("3", { type: "runStarted", outputs: [output] });
    expect(aggregate).toHaveBeenCalledTimes(1);
    const pending = latest();
    expect(pending.nodes[output.port.nodeId].cache).toBe("new");
    expect(pending.runningNodes.has(output.port.nodeId)).toBe(true);
    aggregate.mockClear();
    event("3", { type: "runCancelled" });
    expect(aggregate).not.toHaveBeenCalled();
    const stopped = latest();
    expect(stopped.runningNodes.size).toBe(0);
    expect(stopped.outputs).toBe(pending.outputs);
    // Let the scheduled null summary settle before restoring spies in the next test.
    await vi.waitFor(() => expect(ResultService.getGraphState).toHaveBeenCalled());
  });

  it("does not publish another result update when a terminal event is delivered twice", () => {
    event("1", { type: "runStarted", outputs: [output] });
    event("1", { type: "runCancelled" });
    const listener = vi.fn();
    const unsubscribe = resultQueryRead.subscribe(listener);
    event("1", { type: "runCancelled" });
    expect(listener).not.toHaveBeenCalled();
    unsubscribe();
  });

  it("keeps concurrent pages and page sizes independent until the last consumer releases", async () => {
    const reference = resultReferenceFixture("1");
    const releaseFirst = resultQueryCoordinator.retainPayload(reference);
    const releaseSecond = resultQueryCoordinator.retainPayload(reference);
    const requests = [
      { ...reference, offset: 0, limit: 200 },
      { ...reference, offset: 200, limit: 200 },
      { ...reference, offset: 0, limit: 100 },
    ];
    const settle: Array<() => void> = [];
    vi.spyOn(ResultService, "getPage").mockImplementation(
      (_reference, offset, limit) =>
        new Promise((resolve) => {
          settle.push(() => resolve({ ...page("1"), offset, requestedLimit: limit }));
        }),
    );
    const pending = requests.map((query) => resultQueryCoordinator.loadPage(query));
    settle.reverse().forEach((resolve) => resolve());
    expect(await Promise.all(pending)).toEqual(requests.map(() => ({ status: "published" })));
    releaseFirst();
    for (const query of requests) {
      expect(resultQueryRead.getPage(query)).toMatchObject({
        offset: query.offset,
        requestedLimit: query.limit,
      });
    }
    releaseSecond();
    for (const query of requests) expect(resultQueryRead.getPage(query)).toBeNull();
  });

  it("isolates analysis parameters, failures and recovery for independent consumers", async () => {
    const reference = resultReferenceFixture("1");
    const release = resultQueryCoordinator.retainPayload(reference);
    const first = {
      reference,
      analysis: { kind: "residualPlot" as const, adjacent: false, maxPoints: 2 },
    };
    const second = {
      reference,
      analysis: { kind: "residualPlot" as const, adjacent: false, maxPoints: 3 },
    };
    const service = vi
      .spyOn(ResultService, "analyze")
      .mockImplementation(async (_ref, analysis) => ({
        kind: "residualPlot",
        value: {
          points: Array.from(
            { length: analysis.kind === "residualPlot" ? analysis.maxPoints : 0 },
            (_, index) => ({ observation: index + 1, x: index, y: index, highlighted: false }),
          ),
          sampled: true,
          totalCount: 10,
          matchedCount: 10,
          sampling: "systematic",
          highlightAvailable: true,
        },
      }));
    expect(
      await Promise.all([
        resultQueryCoordinator.loadAnalysis(first),
        resultQueryCoordinator.loadAnalysis(second),
      ]),
    ).toEqual([{ status: "published" }, { status: "published" }]);
    expect(resultQueryRead.getAnalysis(first)?.value).toHaveProperty("points.length", 2);
    expect(resultQueryRead.getAnalysis(second)?.value).toHaveProperty("points.length", 3);
    service.mockRejectedValueOnce(new Error("read failed"));
    await resultQueryCoordinator.loadAnalysis(first);
    expect(resultQueryRead.getFailure({ kind: "analysis", ...first })).not.toBeNull();
    expect(resultQueryRead.getFailure({ kind: "analysis", ...second })).toBeNull();
    await resultQueryCoordinator.loadAnalysis(first);
    expect(resultQueryRead.getFailure({ kind: "analysis", ...first })).toBeNull();
    release();
    expect(resultQueryRead.getAnalysis(first)).toBeNull();
    expect(resultQueryRead.getAnalysis(second)).toBeNull();
  });

  it("evicts values and pages on rerun, rejects late requests, and publishes only the current result", async () => {
    const publishInvalidation = vi.spyOn(invalidationChannel, "publishResultSessionEnd");
    vi.spyOn(ResultService, "getPinResult").mockResolvedValue(descriptor("1"));
    vi.spyOn(ResultService, "getValue").mockResolvedValue({ kind: "sequence", value: [[1]] });
    vi.spyOn(ResultService, "getPage").mockResolvedValue(page("1"));
    await resultQueryCoordinator.loadPinResult(request);
    await resultQueryCoordinator.loadValue({
      executionSessionId: resultSessionFixture,
      resultId: "1",
    });
    const pageRequest = {
      executionSessionId: resultSessionFixture,
      resultId: "1",
      offset: 0,
      limit: 200,
    };
    await resultQueryCoordinator.loadPage(pageRequest);
    let settle!: (value: ResultPage) => void;
    vi.mocked(ResultService.getPage).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          settle = resolve;
        }),
    );
    const pending = resultQueryCoordinator.loadPage(pageRequest);
    event("2", { type: "runStarted", outputs: [output] });
    expect(resultQueryRead.getDescriptor(resultReferenceFixture("1"))).toBeNull();
    expect(resultQueryRead.getValue(resultReferenceFixture("1"))).toBeNull();
    expect(resultQueryRead.getPage(pageRequest)).toBeNull();
    expect(resultQueryRead.getPinResult(request)).toBeNull();
    expect(publishInvalidation).not.toHaveBeenCalled();
    settle(page("1"));
    await expect(pending).resolves.toEqual({ status: "stale" });
    expect(resultQueryRead.getPage(pageRequest)).toBeNull();
    vi.mocked(ResultService.getPinResult).mockResolvedValue(descriptor("2"));
    vi.mocked(ResultService.getGraphState).mockResolvedValue(resultState("2"));
    event("2", { type: "runCompleted" });
    await vi.waitFor(() => expect(resultQueryRead.getPinResult(request)?.resultId).toBe("2"));
    event("1", { type: "runStarted", outputs: [output] });
    expect(resultQueryRead.getPinResult(request)?.resultId).toBe("2");
    vi.mocked(ResultService.getGraphState).mockResolvedValue(resultState(null));
    event("3", { type: "runStarted", outputs: [output] });
    event("3", { type: "runCancelled" });
    expect(resultQueryRead.getPinResult(request)).toBeNull();
    expect(resultQueryRead.getDescriptor(resultReferenceFixture("2"))).toBeNull();
    const nextExecutionSessionId = "00000000-0000-0000-0000-000000000002";
    vi.mocked(ResultService.getPinResult).mockResolvedValue({
      ...descriptor("1"),
      executionSessionId: nextExecutionSessionId,
    });
    vi.mocked(ResultService.getGraphState).mockResolvedValue(
      resultState("1", nextExecutionSessionId),
    );
    installGraphRunEvent({
      resultRevision: "1",
      run: {
        semanticInputHash: "0".repeat(64),
        runId: "1",
        graphPath,
        executionSessionId: nextExecutionSessionId,
      },
      kind: { type: "runStarted", outputs: [output] },
    });
    installGraphRunEvent({
      resultRevision: "1",
      run: {
        semanticInputHash: "0".repeat(64),
        runId: "1",
        graphPath,
        executionSessionId: nextExecutionSessionId,
      },
      kind: { type: "runCompleted" },
    });
    await vi.waitFor(() => expect(resultQueryRead.getPinResult(request)?.resultId).toBe("1"));
    expect(resultQueryRead.getDescriptor(resultReferenceFixture("2"))).toBeNull();
    event("1", { type: "runCancelled" });
    expect(resultQueryRead.getPinResult(request)?.resultId).toBe("1");
  });

  it("keeps held snapshots separate from current pins when a graph changes", async () => {
    vi.spyOn(ResultService, "getPinResult").mockResolvedValue(descriptor("1"));
    await resultQueryCoordinator.loadPinResult(request);
    const reference = resultReferenceFixture("1");
    const release = resultQueryCoordinator.retainPayload(reference);
    vi.spyOn(ResultService, "getValue").mockResolvedValue({ kind: "value", value: 42 });
    vi.spyOn(ResultService, "getDescriptor").mockResolvedValue(descriptor("1"));
    await resultQueryCoordinator.loadValue(reference);
    const changed = structuredClone(fixture.projection);
    changed.basis.semanticInputHash = "1".repeat(64);
    const previous = useResourceStore.getState().sessions[graphPath];
    useResourceStore.getState().installGraphSession(graphPath, {
      ...makeGraphEditorSession(changed),
      resultState: { ...resultState(null), semanticInputHash: changed.basis.semanticInputHash },
    });
    reconcileGraphResultQueries(graphPath, previous);
    expect(resultQueryRead.getPinResult(request)).toBeNull();
    expect(resultQueryRead.getDescriptor(reference)?.resultId).toBe("1");
    expect(resultQueryRead.getValue(reference)).toEqual({ kind: "value", value: 42 });
    await resultQueryCoordinator.loadDescriptor(reference);
    expect(resultQueryRead.getPinResult(request)).toBeNull();
    release();
    expect(resultQueryRead.getDescriptor(reference)).toBeNull();
    useResourceStore.getState().setGraphResultState(graphPath, resultState("1"));
    await resultQueryCoordinator.loadPinResult(request);
    useResourceStore.getState().removeGraphSession(graphPath);
    resetGraphResultQueries(graphPath);
    expect(resultQueryRead.getDescriptor(resultReferenceFixture("1"))).toBeNull();
    expect(resultQueryRead.getPinResult(request)).toBeNull();
  });
  it("keeps independent pages, releases closed-view payloads, and supports detached result reads", async () => {
    clearProjectLifecycle();
    const releasePayload = resultQueryCoordinator.retainPayload(resultReferenceFixture("1"));
    vi.spyOn(ResultService, "getDescriptor").mockResolvedValue(descriptor("1"));
    vi.spyOn(ResultService, "getPage").mockImplementation(async (id, offset, limit) => ({
      ...page(id.resultId),
      offset,
      requestedLimit: limit,
    }));
    await resultQueryCoordinator.loadDescriptor({
      executionSessionId: resultSessionFixture,
      resultId: "1",
    });
    const first = {
      executionSessionId: resultSessionFixture,
      resultId: "1",
      offset: 0,
      limit: 200,
    };
    const second = { ...first, offset: 200 };
    await resultQueryCoordinator.loadPage(first);
    await resultQueryCoordinator.loadPage(second);
    expect(resultQueryRead.getPage(first)?.offset).toBe(0);
    expect(resultQueryRead.getPage(second)?.offset).toBe(200);
    const truncated = {
      ...page("1"),
      offset: 1,
      actualCount: 0,
      values: [],
    };
    vi.mocked(ResultService.getPage).mockResolvedValueOnce(truncated);
    await resultQueryCoordinator.loadPage(second);
    expect(resultQueryRead.getPage(second)).toEqual(truncated);
    expect(resultQueryRead.getPage(first)?.offset).toBe(0);
    let settle!: (value: ResultPage) => void;
    vi.mocked(ResultService.getPage).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          settle = resolve;
        }),
    );
    const pending = resultQueryCoordinator.loadPage(first);
    releasePayload();
    expect(resultQueryRead.getPage(second)).toBeNull();
    expect(resultQueryRead.getDescriptor(resultReferenceFixture("1"))).toBeNull();
    settle(page("1"));
    await expect(pending).resolves.toEqual({ status: "stale" });
    expect(resultQueryRead.getPage(first)).toBeNull();
  });
});
