import { describe, expect, it } from "vitest";
import { resolveNodeInspectionTarget } from "./resolveNodeInspectionTarget";

describe("resolveNodeInspectionTarget", () => {
  it("returns one exact graph-scoped node target", () => {
    expect(resolveNodeInspectionTarget("events/Main.yssbi-event", ["node-1"])).toEqual({
      kind: "node",
      graphPath: "events/Main.yssbi-event",
      nodeId: "node-1",
    });
  });

  it("returns empty when the active editor is not a graph", () => {
    expect(resolveNodeInspectionTarget(null, ["node-1"])).toEqual({ kind: "empty" });
  });
});
