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
    canCopy: true,
    canDelete: true,
    canEditLabel: true,
    canEditParameters: false,
    supportsInlineLiterals: true,
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
      current: 0,
      maximum: direction === "input" ? 1 : null,
      ordered: false,
      canAppend: true,
      canReplace: false,
      canMove: true,
    },
    input:
      direction === "input"
        ? { literalOverride: null, protocolDefault: null, effective: "unbound" }
        : null,
    acceptedType: { display: "Float64", domain: [{ kind: "Float64" }] },
    typeState: { status: "exact", display: "Float64", dataType: { kind: "Float64" } },
    resolvedSchema: null,
    status: "resolved",
  };
}

const inputPin = pin("pin-in", "input", "A");
const outputPin = pin("pin-out", "output", "Result");

describe("toUiNode", () => {
  it("maps projected display and pin slices to a canvas node", () => {
    const view = toUiNode(baseNode, {
      pins: [
        { pin: inputPin, connectionIds: ["connection-1"] },
        { pin: outputPin, connectionIds: ["connection-1"] },
      ],
    });

    expect(view).toMatchObject({
      id: "node-1",
      nodeType: "math.add",
      title: "Projected Add",
      display: baseNode.display,
      parameterEditors: [],
      diagnostics: [],
      styleId: "builtin.default",
    });
    expect(view.inputs[0].connected).toBe(true);
    expect(view.outputs[0].connectionIds).toEqual(["connection-1"]);
  });
});

describe("isRerouteNodeView", () => {
  it("classifies only the Rust-authored builtin.reroute style", () => {
    expect(REROUTE_NODE_STYLE_ID).toBe("builtin.reroute");
    expect(isRerouteNodeView({ styleId: "builtin.reroute" })).toBe(true);
    expect(isRerouteNodeView({ styleId: "reroute" })).toBe(false);
    expect(isRerouteNodeView({ styleId: "builtin.default" })).toBe(false);
  });

  it("preserves projected reroute position and port descriptors without synthesizing identity", () => {
    const view = toUiNode(
      {
        ...baseNode,
        id: "reroute-1",
        nodeType: "opaque.backend.identity",
        position: { x: 135, y: 246 },
        display: { ...baseNode.display!, styleId: "builtin.reroute" },
      },
      {
        pins: [
          {
            pin: {
              ...inputPin,
              id: "projected-address-key",
              nodeId: "reroute-1",
              address: { kind: "declared", nodeId: "reroute-1", portKey: "input" },
              acceptedType: { display: "T", domain: null },
              typeState: { status: "unknown", reasonCode: "unresolved_upstream" },
            },
            connectionIds: ["edge-a"],
          },
        ],
      },
    );

    expect(isRerouteNodeView(view)).toBe(true);
    expect(view.position).toEqual({ x: 135, y: 246 });
    expect(view.nodeType).toBe("opaque.backend.identity");
    expect(view.inputs[0]).toMatchObject({
      id: "projected-address-key",
      address: { kind: "declared", nodeId: "reroute-1", portKey: "input" },
      typeState: { status: "unknown", reasonCode: "unresolved_upstream" },
    });
  });
});
