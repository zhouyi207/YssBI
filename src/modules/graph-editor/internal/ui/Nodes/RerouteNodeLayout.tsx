import type React from "react";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import type { UINode } from "@/features/core/dataStore/nodeView";
import type { GraphContextMenuActions } from "@/features/application/editor";
import {
  REROUTE_GRIP_SIZE_PX,
  REROUTE_NODE_HEIGHT_PX,
  REROUTE_NODE_WIDTH_PX,
} from "@/features/domain/node/utils/nodeClassNames";
import { GraphPinController } from "../Pins/GraphPinController";

export { REROUTE_GRIP_SIZE_PX, REROUTE_NODE_HEIGHT_PX, REROUTE_NODE_WIDTH_PX };

interface RerouteNodeLayoutProps {
  node: UINode;
  activePinId?: string | null;
  graphPath?: string;
  groupId?: string;
  contextMenuActions?: GraphContextMenuActions | null;
  onPinPointerDown?: (event: React.PointerEvent, pin: PinData) => void;
}

export function RerouteNodeLayout({
  node,
  activePinId,
  graphPath,
  groupId,
  contextMenuActions,
  onPinPointerDown,
}: RerouteNodeLayoutProps) {
  const input = node.inputs[0];
  const output = node.outputs[0];

  return (
    <div data-reroute-layout className="relative flex h-full w-full items-center justify-between">
      {input ? (
        <div className="absolute left-0 top-1/2 -translate-x-1/2 -translate-y-1/2 [&_.pin-container]:h-5 [&_.pin-container>span]:hidden [&_.pin-circle]:m-0 [&_.pin-circle]:h-5 [&_.pin-circle]:w-5">
          <GraphPinController
            pin={input}
            graphPath={graphPath}
            groupId={groupId}
            contextMenuActions={contextMenuActions}
            isActive={activePinId === input.id}
            onPinPointerDown={onPinPointerDown}
          />
        </div>
      ) : null}
      <div
        data-reroute-grip
        className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 rounded-full border border-current opacity-70"
        style={{ width: REROUTE_GRIP_SIZE_PX, height: REROUTE_GRIP_SIZE_PX }}
      />
      {output ? (
        <div className="absolute right-0 top-1/2 translate-x-1/2 -translate-y-1/2 [&_.pin-container]:h-5 [&_.pin-container>span]:hidden [&_.pin-circle]:m-0 [&_.pin-circle]:h-5 [&_.pin-circle]:w-5">
          <GraphPinController
            pin={output}
            graphPath={graphPath}
            groupId={groupId}
            contextMenuActions={contextMenuActions}
            isActive={activePinId === output.id}
            onPinPointerDown={onPinPointerDown}
          />
        </div>
      ) : null}
    </div>
  );
}
