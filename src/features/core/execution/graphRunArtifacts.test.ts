import { describe, expect, it } from "vitest";
import { graphHasClearableArtifacts } from "./graphRunArtifacts";
import type { GraphExecutionState } from "./executionTypes";

function graph(partial: Partial<GraphExecutionState>): GraphExecutionState {
  return {
    status: "idle",
    runId: null,
    nodeStates: new Map(),
    completedConnections: new Set(),
    flowingConnections: new Set(),
    recording: [],
    graphDirty: false,
    runFailure: null,
    pinPreviews: new Map(),
    ...partial,
  };
}

describe("graphHasClearableArtifacts", () => {
  it("returns false when idle with no artifacts", () => {
    expect(graphHasClearableArtifacts(undefined)).toBe(false);
    expect(graphHasClearableArtifacts(graph({}))).toBe(false);
  });

  it("returns false while running", () => {
    expect(
      graphHasClearableArtifacts(
        graph({ status: "running", nodeStates: new Map([["n", {} as never]]) }),
      ),
    ).toBe(false);
  });

  it("returns true when recording exists", () => {
    expect(
      graphHasClearableArtifacts(
        graph({ recording: [{ event: { event: "executionStart" }, timestamp: 0 }] }),
      ),
    ).toBe(true);
  });

  it("returns true after completed or error status", () => {
    expect(graphHasClearableArtifacts(graph({ status: "completed" }))).toBe(true);
    expect(graphHasClearableArtifacts(graph({ status: "error" }))).toBe(true);
  });
});
