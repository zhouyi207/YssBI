import { expect, it } from "vitest";
import { projectGraphPresentation } from "./graphPresentation";
import type { GraphResultState } from "@/shared/types/domain/result";
import { portAddressKey } from "@/features/domain/editorProjection";

it("shares unaffected element projections and reuses equivalent query results", () => {
  const cache: GraphResultState = {
    revision: "0",

    executionSessionId: "session",
    semanticInputHash: "hash",
    outputs: ["a", "b"].map((nodeId) => ({
      output: {
        graphPath: "events/main",
        port: { kind: "declared" as const, nodeId, portKey: "value" },
      },
      state: "valid" as const,
      resultId: nodeId,
    })),
    connections: [],
  };
  const first = projectGraphPresentation(cache, [], null);
  const running = projectGraphPresentation(cache, ["a"], null, first);
  expect(running.nodes).toBe(first.nodes);
  expect(running.outputs).toBe(first.outputs);
  expect(running.runningNodes.has("a")).toBe(true);
  const changed = {
    ...cache,
    outputs: cache.outputs.map((entry) =>
      entry.output.port.nodeId === "b" ? { ...entry, state: "stale" as const } : entry,
    ),
  };
  const second = projectGraphPresentation(changed, ["a"], null, running);
  expect(second.nodes.a).toBe(first.nodes.a);
  expect(second.nodes.b).not.toBe(first.nodes.b);
  expect(second.runningNodes).toBe(running.runningNodes);
  expect(projectGraphPresentation(structuredClone(changed), ["a"], null, second)).toBe(second);
});

it("masks outputs invalidated by run admission until their current result summary arrives", () => {
  const cache: GraphResultState = {
    executionSessionId: "session",
    semanticInputHash: "hash",
    revision: "1",
    outputs: ["a", "b"].map((nodeId) => ({
      output: { graphPath: "events/Main", port: { kind: "declared", nodeId, portKey: "value" } },
      state: "valid",
      resultId: nodeId,
    })),
    connections: [],
  };
  const previous = projectGraphPresentation(cache, [], null);
  const pending = new Set([portAddressKey(cache.outputs[0].output.port)]);
  const duringRun = projectGraphPresentation(cache, ["a"], null, previous, pending);
  const afterCancel = projectGraphPresentation(cache, [], null, duringRun, pending);
  expect(afterCancel.nodes.a).toEqual({ total: 1, valid: 0, stale: 0, cache: "new" });
  expect(afterCancel.nodes.b).toBe(previous.nodes.b);
  const confirmed = projectGraphPresentation(
    {
      ...cache,
      revision: "2",
      outputs: cache.outputs.map((entry, i) =>
        i === 0 ? { ...entry, state: "missing", resultId: null } : entry,
      ),
    },
    [],
    null,
    afterCancel,
  );
  expect(confirmed.nodes).toBe(afterCancel.nodes);
});
