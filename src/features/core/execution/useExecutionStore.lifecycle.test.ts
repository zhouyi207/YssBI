import { beforeEach, describe, expect, it } from "vitest";
import { useExecutionStore } from "./useExecutionStore";

describe("useExecutionStore run lifecycle", () => {
  beforeEach(() => {
    useExecutionStore.setState({
      graphs: {},
    });
  });

  it("tracks the active opaque run ID only for the live run lifecycle", () => {
    const graphPath = "events/Main.yssbi-event";
    const store = useExecutionStore.getState();

    store.startExecution(graphPath);
    expect(useExecutionStore.getState().getGraph(graphPath).runId).toBeNull();

    store.setActiveRunId(graphPath, "9007199254740993");
    expect(useExecutionStore.getState().getGraph(graphPath).runId).toBe("9007199254740993");

    store.completeExecution(graphPath);
    expect(useExecutionStore.getState().getGraph(graphPath).runId).toBeNull();
  });

  it("revokes a run callback when an edit clears its request or a new run replaces it", () => {
    const graphPath = "events/Main.yssbi-event";
    const store = useExecutionStore.getState();
    const old = store.startExecution(graphPath);
    expect(old()).toBe(true);
    store.clearGraphRunProjections(graphPath);
    expect(old()).toBe(false);
    const current = store.startExecution(graphPath);
    expect(old()).toBe(false);
    expect(current()).toBe(true);
    store.startExecution(graphPath);
    expect(current()).toBe(false);
  });

  it("releases execution UI state when a graph tab is fully closed", () => {
    const graphPath = "events/Main.yssbi-event";
    const store = useExecutionStore.getState();
    store.completeExecution(graphPath);

    store.releaseGraphExecutionState(graphPath);

    expect(useExecutionStore.getState().graphs[graphPath]).toBeUndefined();
  });
});
