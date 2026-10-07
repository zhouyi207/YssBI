import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useExecutionStore } from "@/features/core/execution";
import type { RunEvent } from "@/shared/types/domain/runEvent";
import { installGraphRunEvent } from "./observeGraphRunEvent";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { resetResultQueryProject } from "@/features/application/results/runtime";
import * as graphPresentation from "@/features/application/results/graphPresentation";
import { ResultService } from "@/services/result/resultService";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";

function event(kind: RunEvent["kind"]): RunEvent {
  return {
    resultRevision: "1",
    run: {
      semanticInputHash: "0".repeat(64),
      executionSessionId: "project-session-1",
      graphPath: "events/Main.yssbi-event",
      runId: "9007199254740993",
    },
    kind,
  };
}

describe("observeGraphRunEvent", () => {
  it("installs unknown runs from recovery and ignores duplicate starts and older terminal snapshots", () => {
    useExecutionStore.setState({ graphs: {} });
    const started = event({ type: "runStarted", outputs: [] });
    started.run = { ...started.run, executionSessionId: "recovered-session", runId: "51" };
    const path = started.run.graphPath;
    expect(installGraphRunEvent(started)).toBe(true);
    useExecutionStore.getState().markExecutionUnknown(path);
    installGraphRunEvent(started);
    expect(useExecutionStore.getState().getGraph(path).status).toBe("running");
    installGraphRunEvent({ ...started, kind: { type: "runCompleted" } });
    installGraphRunEvent(started);
    expect(useExecutionStore.getState().getGraph(path).status).toBe("completed");
    const next = { ...started, run: { ...started.run, runId: "52" } };
    installGraphRunEvent(next);
    installGraphRunEvent({ ...started, kind: { type: "runCompleted" } });
    expect(useExecutionStore.getState().getGraph(path)).toMatchObject({
      status: "running",
      run: { runId: "52" },
    });
    const output = makeEditorProjectionFixture({ graphPath: path }).outputAddress;
    const firstOutput = { graphPath: path, port: output };
    const secondOutput = { graphPath: path, port: { ...output, portKey: "second" } };
    const overlapping = {
      ...started,
      run: { ...started.run, runId: "53" },
      kind: { type: "runStarted" as const, outputs: [firstOutput] },
    };
    installGraphRunEvent(overlapping);
    const latest = {
      ...overlapping,
      run: { ...overlapping.run, runId: "54" },
      kind: { type: "runStarted" as const, outputs: [secondOutput] },
    };
    installGraphRunEvent(latest);
    installGraphRunEvent({ ...overlapping, kind: { type: "runCompleted" } });
    const current = useExecutionStore.getState().getGraph(path);
    expect(current.run?.runId).toBe("54");
    expect(
      Object.values(current.outputRuns).map((run) => ({
        runId: run.run.runId,
        active: run.active,
      })),
    ).toEqual([
      { runId: "53", active: false },
      { runId: "54", active: true },
    ]);
  });
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(ResultService, "getGraphState").mockResolvedValue(null);
    resetResultQueryProject();
    startProjectLifecycle("observation-test");
    useExecutionStore.setState({
      graphs: {},
    });
    useResourceStore.getState().clear();
  });

  it("rejects a delayed run from another semantic basis instead of assigning the current graph identity", () => {
    const path = "events/Main.yssbi-event";
    const fixture = makeEditorProjectionFixture({ graphPath: path });
    fixture.projection.basis.semanticInputHash = "b".repeat(64);
    const session = makeGraphEditorSession(fixture.projection);
    session.resultState = { ...session.resultState, executionSessionId: "project-session-1" };
    useResourceStore.getState().installGraphSession(path, session);
    const started = event({
      type: "runStarted",
      outputs: [{ graphPath: path, port: fixture.outputAddress }],
    });
    const delayed = { ...started, run: { ...started.run, semanticInputHash: "a".repeat(64) } };
    expect.soft(installGraphRunEvent(delayed)).toBe(false);
    expect.soft(useExecutionStore.getState().graphs[path]).toBeUndefined();
    expect(installGraphRunEvent({ ...delayed, kind: { type: "runCompleted" } })).toBe(false);
    const inspection: RunEvent = {
      ...delayed,
      kind: {
        type: "resultInspectionRequested",
        resultId: "1",
        source: { graphPath: path, nodeId: fixture.outputAddress.nodeId, portAddress: null },
      },
    };
    expect(installGraphRunEvent(inspection)).toBe(true);
    expect(installGraphRunEvent(inspection)).toBe(false);
    expect(useExecutionStore.getState().graphs[path]).toBeUndefined();
    const current = {
      ...started,
      run: { ...started.run, semanticInputHash: "b".repeat(64), runId: "9007199254740994" },
    };
    expect(installGraphRunEvent(current)).toBe(true);
    expect(useExecutionStore.getState().getGraph(path).run?.runId).toBe(current.run.runId);
  });

  it("publishes the runStarted identity and output activity together", () => {
    const graphPath = "events/Main.yssbi-event";
    useExecutionStore.getState().submitExecution(graphPath);

    const observed: unknown[] = [];
    const stop = useExecutionStore.subscribe((state) => {
      const graph = state.graphs[graphPath];
      observed.push({
        runId: graph.run?.runId,
        active: Object.values(graph.outputRuns).filter((run) => run.active).length,
      });
    });
    installGraphRunEvent(
      event({
        type: "runStarted",
        outputs: [{ graphPath, port: makeEditorProjectionFixture({ graphPath }).outputAddress }],
      }),
    );
    stop();
    expect(observed).toEqual([{ runId: "9007199254740993", active: 1 }]);

    expect(useExecutionStore.getState().getGraph(graphPath).run?.runId).toBe("9007199254740993");
  });

  it("classifies canonical terminal events", () => {
    const graphPath = "events/Main.yssbi-event";
    const fixture = makeEditorProjectionFixture({ graphPath });
    const session = makeGraphEditorSession(fixture.projection);
    session.resultState = { ...session.resultState, executionSessionId: "project-session-1" };
    useResourceStore.getState().installGraphSession(graphPath, session);
    useExecutionStore.getState().submitExecution(graphPath);
    installGraphRunEvent(
      event({ type: "runStarted", outputs: [{ graphPath, port: fixture.outputAddress }] }),
    );
    useExecutionStore.getState().markExecutionUnknown(graphPath);
    const compose = vi.spyOn(graphPresentation, "projectGraphPresentation");

    const observed: unknown[] = [];
    const stop = useExecutionStore.subscribe((state) => {
      const graph = state.graphs[graphPath];
      observed.push({ status: graph.status, failure: graph.runFailure?.code });
    });
    installGraphRunEvent(
      event({
        type: "runErrored",
        groups: [],
        code: "kernelFailed",
        phase: "execution",
        source: null,
      }),
    );
    stop();
    expect(observed).toEqual([{ status: "error", failure: "kernelFailed" }]);
    expect(
      compose.mock.results.map((result) =>
        result.type === "return"
          ? {
              running: result.value.runningNodes.size,
              failure: result.value.failure?.code,
            }
          : null,
      ),
    ).toEqual([{ running: 0, failure: "kernelFailed" }]);
    expect(useExecutionStore.getState().getGraph(graphPath).runFailure).toMatchObject({
      code: "kernelFailed",
      phase: "execution",
      run: { runId: "9007199254740993" },
    });

    expect(useExecutionStore.getState().getGraph(graphPath).status).toBe("error");
    const nextRun = {
      ...event({ type: "runStarted", outputs: [] }),
      run: { ...event({ type: "runCancelled" }).run, runId: "9007199254740994" },
    };
    installGraphRunEvent(nextRun);
    installGraphRunEvent({ ...nextRun, kind: { type: "runCancelled" } });
    expect(useExecutionStore.getState().getGraph(graphPath).status).toBe("idle");
  });
});
