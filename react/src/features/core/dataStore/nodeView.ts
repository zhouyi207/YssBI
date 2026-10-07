/**
 * Store `NodeData` + pin 切片 → 画布 `UINode` 单点桥接
 */

import type { NodeData, PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import type { GraphEntitiesState, GraphEntityBucket } from "./graphEntityAccess";

export interface UINode extends Pick<
  NodeData,
  "id" | "display" | "parameterGroups" | "diagnostics" | "capabilities"
> {
  inputs: PinData[];
  outputs: PinData[];
}

export const REROUTE_NODE_STYLE_ID = "builtin.reroute";

export function isRerouteNodeView(node: Pick<UINode, "display">): boolean {
  return node.display.styleId === REROUTE_NODE_STYLE_ID;
}

function createOrReuseNodeView(
  nodeData: NodeData,
  inputs: PinData[],
  outputs: PinData[],
  previous: UINode | null,
): UINode {
  if (
    previous &&
    previous.id === nodeData.id &&
    previous.display === nodeData.display &&
    previous.parameterGroups === nodeData.parameterGroups &&
    previous.diagnostics === nodeData.diagnostics &&
    previous.capabilities === nodeData.capabilities &&
    previous.inputs === inputs &&
    previous.outputs === outputs
  )
    return previous;
  return {
    id: nodeData.id,
    display: nodeData.display,
    parameterGroups: nodeData.parameterGroups,
    diagnostics: nodeData.diagnostics,
    capabilities: nodeData.capabilities,
    inputs,
    outputs,
  };
}

/** Group the node's ordered pins, retaining each unchanged side independently. */
export function toUiNode(
  nodeData: NodeData,
  pins: GraphEntityBucket["pins"],
  previous: UINode | null = null,
): UINode {
  let inputs = previous?.inputs ?? [];
  let outputs = previous?.outputs ?? [];
  let inputIndex = 0;
  let outputIndex = 0;
  for (const id of nodeData.pinIds) {
    const pin = pins[id];
    if (!pin) continue;
    if (pin.direction === "output") {
      if (outputs[outputIndex] !== pin) {
        if (outputs === previous?.outputs) outputs = outputs.slice(0, outputIndex);
        outputs[outputIndex] = pin;
      }
      outputIndex++;
    } else {
      if (inputs[inputIndex] !== pin) {
        if (inputs === previous?.inputs) inputs = inputs.slice(0, inputIndex);
        inputs[inputIndex] = pin;
      }
      inputIndex++;
    }
  }
  if (inputs.length !== inputIndex) inputs = inputs.slice(0, inputIndex);
  if (outputs.length !== outputIndex) outputs = outputs.slice(0, outputIndex);
  return createOrReuseNodeView(nodeData, inputs, outputs, previous);
}

/** One subscription's content cache; coordinates belong to the canvas projection. */
export function createNodeViewSelector(nodeId: string, graphPath?: string) {
  let previousNode: NodeData | undefined;
  let previousPins: GraphEntityBucket["pins"] | undefined;
  let view: UINode | null = null;

  return (state: GraphEntitiesState): UINode | null => {
    const bucket = graphPath ? state.graphEntities[graphPath] : undefined;
    const node = bucket?.nodes[nodeId];
    if (!node || !bucket) {
      previousNode = undefined;
      previousPins = undefined;
      return (view = null);
    }
    if (node === previousNode && bucket.pins === previousPins) return view;

    const pinsUnchanged = node.pinIds === previousNode?.pinIds && bucket.pins === previousPins;
    previousNode = node;
    previousPins = bucket.pins;

    return (view =
      view && pinsUnchanged
        ? createOrReuseNodeView(node, view.inputs, view.outputs, view)
        : toUiNode(node, bucket.pins, view));
  };
}
