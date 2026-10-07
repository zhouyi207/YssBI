import { beforeEach, describe, expect, it } from "vitest";
import { useExecutionStore } from "./useExecutionStore";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
const run = {
  graphPath: "events/Main.yssbi-event",
  executionSessionId: "session",
  runId: "9007199254740993",
  semanticInputHash: "0".repeat(64),
};

describe("useExecutionStore run lifecycle", () => {
  beforeEach(() => {
    useExecutionStore.setState({
      graphs: {},
    });
  });

  it("tracks the active run identity and rejects another session's same-ID terminal event", () => {
    const graphPath = "events/Main.yssbi-event";
    const store = useExecutionStore.getState();

    store.submitExecution(graphPath);
    expect(useExecutionStore.getState().getGraph(graphPath).run).toBeNull();

    store.applyRunEvent({ resultRevision: "1", run, kind: { type: "runStarted", outputs: [] } });
    expect(useExecutionStore.getState().getGraph(graphPath).run).toEqual(run);

    const { outputAddress } = makeEditorProjectionFixture({ graphPath });
    const first = { graphPath, port: outputAddress };
    const second = {
      graphPath,
      port: { ...outputAddress, nodeId: "00000000-0000-0000-0000-000000000099" },
    };
    store.applyRunEvent({
      resultRevision: "2",
      run,
      kind: { type: "runStarted", outputs: [first] },
    });
    const priorOutput = Object.values(
      useExecutionStore.getState().getGraph(graphPath).outputRuns,
    )[0];
    const continuation = {
      resultRevision: "4",
      run,
      kind: { type: "runStarted" as const, outputs: [first, second] },
    };
    expect(store.getRunEventOutputs(continuation)).toEqual([second]);
    store.applyRunEvent(continuation);
    expect(Object.values(useExecutionStore.getState().getGraph(graphPath).outputRuns)).toContain(
      priorOutput,
    );
    expect(Object.values(useExecutionStore.getState().getGraph(graphPath).outputRuns)).toHaveLength(
      2,
    );
    expect(useExecutionStore.getState().getGraph(graphPath).status).toBe("running");

    const successor = { ...run, executionSessionId: "successor-session" };
    store.applyRunEvent({
      resultRevision: "1",
      run: successor,
      kind: { type: "runStarted", outputs: [] },
    });
    store.applyRunEvent({ resultRevision: "1", run, kind: { type: "runCompleted" } });
    expect(useExecutionStore.getState().getGraph(graphPath).status).toBe("running");
    store.applyRunEvent({ resultRevision: "1", run: successor, kind: { type: "runCompleted" } });
    expect(useExecutionStore.getState().getGraph(graphPath).run).toBeNull();
  });

  it("revokes a run callback when an edit clears its request or a new run replaces it", () => {
    const graphPath = "events/Main.yssbi-event";
    const store = useExecutionStore.getState();
    const old = store.submitExecution(graphPath);
    expect(old()).toBe(true);
    store.clearGraphRunProjections(graphPath);
    expect(old()).toBe(false);
    const current = store.submitExecution(graphPath);
    expect(old()).toBe(false);
    expect(current()).toBe(true);
    store.submitExecution(graphPath);
    expect(current()).toBe(false);
  });

  it("releases execution UI state when a graph tab is fully closed", () => {
    const graphPath = "events/Main.yssbi-event";
    const store = useExecutionStore.getState();
    store.applyRunEvent({ resultRevision: "1", run, kind: { type: "runStarted", outputs: [] } });
    store.applyRunEvent({ resultRevision: "1", run, kind: { type: "runCompleted" } });

    store.releaseGraphExecutionState(graphPath);

    expect(useExecutionStore.getState().graphs[graphPath]).toBeUndefined();
  });

  it("does not republish repeated unknown or cleared states and retains other graphs", () => {
    const store = useExecutionStore.getState();
    const otherPath = "events/Other.yssbi-event";
    store.submitExecution(otherPath);
    const other = useExecutionStore.getState().graphs[otherPath];
    let notifications = 0;
    const stop = useExecutionStore.subscribe(() => notifications++);
    try {
      store.markExecutionUnknown(run.graphPath);
      const unknown = useExecutionStore.getState();
      store.markExecutionUnknown(run.graphPath);
      expect(useExecutionStore.getState()).toBe(unknown);
      expect(notifications).toBe(1);

      store.clearGraphRunProjections(run.graphPath);
      const cleared = useExecutionStore.getState();
      store.clearGraphRunProjections(run.graphPath);
      expect(useExecutionStore.getState()).toBe(cleared);
      expect(notifications).toBe(2);
      expect(cleared.graphs[otherPath]).toBe(other);
      expect(unknown.graphs[run.graphPath].status).toBe("unknown");
    } finally {
      stop();
    }
  });
});
