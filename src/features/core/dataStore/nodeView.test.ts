import { describe, expect, it } from "vitest";
import type { NodeData, PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { isRerouteNodeView, REROUTE_NODE_STYLE_ID, toUiNode } from "./nodeView";

const baseNode: NodeData = {
  id: "node-1",
  graphPath: "graph-1",
  nodeType: "math.add",
  pinIds: ["pin-in", "pin-out"],
  position: { x: 10, y: 20 },
  display: {
    title: "Projected Add",
    userLabel: null,
    iconId: null,
    styleId: "builtin.default",
  },
  parameterEditors: [],
  portInstanceAdditions: [],
  capabilities: {
    managed: false,
  },
  diagnostics: [],
};

function pin(id: string, direction: "input" | "output", label: string): PinData {
  return {
    id,
    nodeId: "node-1",
    name: label,
    direction,
    address: { kind: "declared", nodeId: "node-1", portKey: id },
    display: { label, instanceLabel: null },
    orphan: false,
    canRemove: false,
    connections: {
      current: 1,
      maximum: direction === "input" ? 1 : null,
      ordered: false,
      canAppend: direction === "output",
      canReplace: direction === "input",
      canMove: true,
    },
    input:
      direction === "input"
        ? {
            literalOverride: null,
            protocolDefault: null,
            effective: "connections",
          }
        : null,
    acceptedType: { display: "Float64", domain: [{ kind: "Scalar", inner: "Numeric" }] },
    typeState: {
      status: "exact",
      display: "Float64",
      dataType: { kind: "Scalar", inner: "Numeric" },
    },
    resolvedSchema: null,
    status: "resolved",
  };
}

const inputPin = pin("pin-in", "input", "A");
const outputPin = pin("pin-out", "output", "Result");

describe("toUiNode", () => {
  it("reuses projected pins and their connection facts in the canvas node", () => {
    const view = toUiNode(baseNode, [inputPin, outputPin]);

    expect(view).toMatchObject({
      id: "node-1",
      display: baseNode.display,
      parameterEditors: [],
      diagnostics: [],
      capabilities: baseNode.capabilities,
    });
    expect(view.inputs[0]).toBe(inputPin);
    expect(view.outputs[0]).toBe(outputPin);
    expect(view.inputs[0].connections.current).toBe(1);
  });
});

describe("isRerouteNodeView", () => {
  it("classifies only the Rust-authored builtin.reroute style", () => {
    expect(REROUTE_NODE_STYLE_ID).toBe("builtin.reroute");
    expect(
      isRerouteNodeView({ display: { ...baseNode.display, styleId: "builtin.reroute" } }),
    ).toBe(true);
    expect(isRerouteNodeView({ display: { ...baseNode.display, styleId: "reroute" } })).toBe(false);
    expect(isRerouteNodeView({ display: baseNode.display })).toBe(false);
  });
});
