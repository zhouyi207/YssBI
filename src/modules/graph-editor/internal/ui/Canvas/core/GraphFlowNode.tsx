import { memo, useLayoutEffect, useRef } from "react";
import { Handle, Position, useUpdateNodeInternals, type NodeProps } from "@xyflow/react";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { GraphNodeController } from "../../Nodes/GraphNodeController";
import { useGraphFlowContext, useGraphFlowInteraction } from "./GraphFlowContext";
import { getFlowPinFeedback, type GraphFlowNode as FlowNode } from "./graphFlowModel";

function GraphFlowHandle({ pin }: { pin: PinData }) {
  const { interactive } = useGraphFlowContext();
  const feedback = useGraphFlowInteraction((state) =>
    state.targetId === pin.id ? getFlowPinFeedback(state, pin.id) : null,
  );
  const feedbackClass = feedback
    ? {
        invalid: "ring-2 ring-red-500/90",
        replace: "ring-2 ring-amber-500/90",
        append: "ring-2 ring-emerald-500/90",
        pending: "",
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
  const previousHandlesKey = useRef(data.handlesKey);
  // React Flow's shared ResizeObserver measures mounts and size changes in one batch.
  // Subsequent handle changes may keep the same size and need an explicit refresh.
  useLayoutEffect(() => {
    if (previousHandlesKey.current === data.handlesKey) return;
    previousHandlesKey.current = data.handlesKey;
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
