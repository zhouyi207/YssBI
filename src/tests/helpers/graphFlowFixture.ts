import type { GraphEntityBucket } from "@/features/core/dataStore/graphEntityAccess";
import { makeEditorProjectionFixture, makeProjectedPinData } from "./editorProjectionFixtures";

export const FLOW_GRAPH_PATH = "events/flow";

export function makeGraphFlowFixture(): GraphEntityBucket {
  const base = makeEditorProjectionFixture({ graphPath: FLOW_GRAPH_PATH }).projection;
  const out = makeProjectedPinData({ id: "out", nodeId: "source", direction: "output" });
  const left = makeProjectedPinData({ id: "left", nodeId: "target", direction: "input" });
  const right = makeProjectedPinData({ id: "right", nodeId: "target", direction: "input" });
  out.connections.current = 1;
  left.connections = { ...left.connections, current: 1, canAppend: false, canReplace: true };
  return {
    basis: base.basis,
    outcome: base.outcome,
    diagnostics: [],
    hasBlockingDiagnostics: false,
    graphNodes: ["source", "target", "managed"],
    nodes: Object.fromEntries(
      ["source", "target", "managed"].map((id, index) => [
        id,
        {
          id,
          graphPath: FLOW_GRAPH_PATH,
          nodeType: "tests.node",
          pinIds: id === "source" ? ["out"] : id === "target" ? ["left", "right"] : [],
          position: { x: index * 250, y: 0 },
          display: { ...base.nodes[0].display, title: id },
          parameterEditors: [],
          portInstanceAdditions: [],
          diagnostics: [],
          capabilities: { ...base.nodes[0].capabilities, managed: id === "managed" },
        },
      ]),
    ),
    pins: { out, left, right },
    connections: { original: { id: "original", from: "out", to: "left" } },
    pinConnections: { out: ["original"], left: ["original"], right: [] },
  };
}
