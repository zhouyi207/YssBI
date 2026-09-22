import { expect, it } from "vitest";
import { projectGraphPresentation } from "./graphPresentation";
import type { GraphResultCacheProjection } from "./runtime";

it("shares unaffected element projections and reuses equivalent query results", () => {
  const cache: GraphResultCacheProjection = {
    executionSessionId: "session",
    semanticInputHash: "hash",
    outputs: Object.fromEntries(
      ["a", "b"].map((nodeId) => [
        nodeId,
        {
          output: {
            graphPath: "events/main",
            port: { kind: "declared" as const, nodeId, portKey: "value" },
          },
          state: "valid" as const,
          resultId: nodeId,
        },
      ]),
    ),
    connections: [],
  };
  const first = projectGraphPresentation(cache, [], null);
  const running = projectGraphPresentation(cache, ["a"], null, first);
  expect(running.nodes).toBe(first.nodes);
  expect(running.outputs).toBe(first.outputs);
  expect(running.runningNodes.has("a")).toBe(true);
  const changed = {
    ...cache,
    outputs: { ...cache.outputs, b: { ...cache.outputs.b, state: "stale" as const } },
  };
  const second = projectGraphPresentation(changed, ["a"], null, running);
  expect(second.nodes.a).toBe(first.nodes.a);
  expect(second.nodes.b).not.toBe(first.nodes.b);
  expect(second.runningNodes).toBe(running.runningNodes);
  expect(projectGraphPresentation(structuredClone(changed), ["a"], null, second)).toBe(second);
});
