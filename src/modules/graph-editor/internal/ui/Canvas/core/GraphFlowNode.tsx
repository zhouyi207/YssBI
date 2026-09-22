import { memo, useLayoutEffect } from "react";
import { Handle, Position, useUpdateNodeInternals, type NodeProps } from "@xyflow/react";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { GraphNodeController } from "../../Nodes/GraphNodeController";
import { useGraphFlowContext, useGraphFlowInteraction } from "./GraphFlowContext";
import type { GraphFlowNode as FlowNode } from "./graphFlowModel";

function GraphFlowHandle({ pin }: { pin: PinData }) {
  const { interactive } = useGraphFlowContext();
  const feedback = useGraphFlowInteraction((state) =>
    state.targetId === pin.id ? (state.pins[pin.id]?.feedback ?? null) : null,
  );
  const feedbackClass = feedback
    ? {
        invalid: "ring-2 ring-red-500/90",
        replace: "ring-2 ring-amber-500/90",
        append: "ring-2 ring-emerald-500/90",
      }[feedback.kind]
    : "";
  return (
    <Handle
      id={pin.id}
      // Loose handles let the Rust projection own direction, including damaged links shown for repair.
      type="source"
      position={pin.direction === "input" ? Position.Left : Position.Right}
      isConnectable={interactive && !pin.orphan}
      isConnectableStart={
        pin.connections.canAppend || pin.connections.canReplace || pin.connections.canMove
      }
      className={`yss-flow-handle ${feedbackClass}`}
      data-connection-feedback={feedback?.kind}
      data-connection-invalid-reason={feedback?.kind === "invalid" ? feedback.reason : undefined}
    />
  );
}

const renderHandle = (pin: PinData) => <GraphFlowHandle pin={pin} />;

export const GraphFlowNode = memo(function GraphFlowNode({
  id,
  data,
  selected,
}: NodeProps<FlowNode>) {
  const { graphPath, groupId, contextMenuActions } = useGraphFlowContext();
  const updateNodeInternals = useUpdateNodeInternals();
  // Reordering equal-sized dynamic rows does not trigger ResizeObserver.
  useLayoutEffect(() => {
    updateNodeInternals(id);
  }, [id, data.handlesKey, updateNodeInternals]);
  return (
    <GraphNodeController
      id={id}
      graphPath={graphPath}
      groupId={groupId}
      selected={selected}
      contextMenuActions={contextMenuActions}
      renderPinHandle={renderHandle}
    />
  );
});
