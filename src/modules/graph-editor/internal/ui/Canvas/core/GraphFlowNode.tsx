import { memo, useCallback, useLayoutEffect } from "react";
import {
  Handle,
  Position,
  useConnection,
  useUpdateNodeInternals,
  type NodeProps,
} from "@xyflow/react";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { GraphNodeController } from "../../Nodes/GraphNodeController";
import {
  pinConnectionFeedbackAttributes,
  pinConnectionFeedbackClass,
} from "../../Pins/GraphPinView";
import { useGraphFlowContext } from "./GraphFlowContext";
import type { GraphFlowNode as FlowNode } from "./graphFlowModel";

function GraphFlowHandle({ pin }: { pin: PinData }) {
  const { interactive, feedbackForPin } = useGraphFlowContext();
  const targeted = useConnection((connection) => connection.toHandle?.id === pin.id);
  const feedback = targeted ? feedbackForPin(pin.id) : null;
  const viewFeedback =
    feedback?.kind === "invalid"
      ? { kind: feedback.kind, invalidReason: feedback.reason }
      : feedback;
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
      className={`yss-flow-handle ${pinConnectionFeedbackClass(viewFeedback)}`}
      {...pinConnectionFeedbackAttributes(viewFeedback)}
    />
  );
}

const renderHandle = (pin: PinData) => <GraphFlowHandle pin={pin} />;

export const GraphFlowNode = memo(function GraphFlowNode({
  id,
  data,
  selected,
}: NodeProps<FlowNode>) {
  const { graphPath, groupId, sourcePin, contextMenuActions, feedbackForPin } =
    useGraphFlowContext();
  const updateNodeInternals = useUpdateNodeInternals();
  // Reordering equal-sized dynamic rows does not trigger ResizeObserver.
  useLayoutEffect(() => {
    updateNodeInternals(id);
  }, [id, data.handlesKey, updateNodeInternals]);
  const canConnectPin = useCallback(
    (pin: PinData) => feedbackForPin(pin.id)?.kind !== "invalid",
    [feedbackForPin],
  );
  return (
    <GraphNodeController
      id={id}
      graphPath={graphPath}
      groupId={groupId}
      selected={selected}
      activePin={sourcePin}
      canConnectPin={canConnectPin}
      contextMenuActions={contextMenuActions}
      renderPinHandle={renderHandle}
    />
  );
});
