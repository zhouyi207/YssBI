import { describe, expect, it } from "vitest";
import { graphHasClearableArtifacts } from "./graphRunArtifacts";
import type { GraphExecutionState } from "./executionTypes";

function graph(partial: Partial<GraphExecutionState>): GraphExecutionState {
  return {
    status: "idle",
    runId: null,
    request: null,
    runFailure: null,
    ...partial,
  };
}

describe("graphHasClearableArtifacts", () => {
  it("returns false when idle with no artifacts", () => {
    expect(graphHasClearableArtifacts(undefined)).toBe(false);
    expect(graphHasClearableArtifacts(graph({}))).toBe(false);
  });

  it("returns false while running", () => {
    expect(graphHasClearableArtifacts(graph({ status: "running" }))).toBe(false);
  });

  it("returns true after completed or error status", () => {
    expect(graphHasClearableArtifacts(graph({ status: "completed" }))).toBe(true);
    expect(graphHasClearableArtifacts(graph({ status: "error" }))).toBe(true);
  });
});
