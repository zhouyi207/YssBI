import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { beforeEach, describe, expect, it } from "vitest";
import { useExecutionStore } from "@/features/core/execution";
import type { RunEvent } from "@/shared/types/domain/runEvent";
import { installGraphRunEvent } from "./observeGraphRunEvent";

function event(kind: RunEvent["kind"]): RunEvent {
  return {
    run: {
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
      runId: "52",
    });
  });
  beforeEach(() => {
    startProjectLifecycle("observation-test");
    useExecutionStore.setState({
      graphs: {},
    });
  });

  it("projects the runStarted opaque ID into the active graph state", () => {
    const graphPath = "events/Main.yssbi-event";
    useExecutionStore.getState().startExecution(graphPath);

    installGraphRunEvent(event({ type: "runStarted", outputs: [] }));

    expect(useExecutionStore.getState().getGraph(graphPath).runId).toBe("9007199254740993");
  });

  it("classifies canonical terminal events", () => {
    const graphPath = "events/Main.yssbi-event";
    useExecutionStore.getState().startExecution(graphPath);
    installGraphRunEvent(event({ type: "runStarted", outputs: [] }));

    installGraphRunEvent(
      event({ type: "runErrored", code: "kernelFailed", phase: "execution", source: null }),
    );
    expect(useExecutionStore.getState().getGraph(graphPath).runFailure).toMatchObject({
      code: "kernelFailed",
      phase: "execution",
      runId: "9007199254740993",
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
