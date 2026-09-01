import type React from "react";
import type { Pin as PinModel } from "@/shared/types/domain";
import type { PortKindDto } from "@/shared/types/domain/editorProjection";
import type { UINode } from "@/shared/types/ui";
import {
  REROUTE_GRIP_SIZE_PX,
  REROUTE_NODE_HEIGHT_PX,
  REROUTE_NODE_WIDTH_PX,
} from "@/features/domain/node/utils/nodeClassNames";
import { Pin } from "../Pins/Pin";

export { REROUTE_GRIP_SIZE_PX, REROUTE_NODE_HEIGHT_PX, REROUTE_NODE_WIDTH_PX };

interface RerouteNodeLayoutProps {
  node: UINode;
  activePinId?: string | null;
  graphPath?: string;
  groupId?: string;
  onPinClick?: (pinId: string, direction: "input" | "output") => void;
  onPinPointerDown?: (event: React.PointerEvent, pin: PinModel) => void;
}

function projectedRerouteKind(node: UINode): PortKindDto | undefined {
  return node.inputs[0]?.kind ?? node.outputs[0]?.kind;
}

function gripClassName(kind: PortKindDto | undefined): string {
  const semanticShape =
    kind === "effect"
      ? "rotate-45 rounded-none"
      : kind === "control"
        ? "rounded-none"
        : "rounded-full";
  return `absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 border border-current opacity-70 ${semanticShape}`;
}

export function RerouteNodeLayout({
  node,
  activePinId,
  graphPath,
  groupId,
  onPinClick,
  onPinPointerDown,
}: RerouteNodeLayoutProps) {
  const input = node.inputs[0];
  const output = node.outputs[0];
  const kind = projectedRerouteKind(node);

  return (
    <div
      data-reroute-layout
      data-reroute-kind={kind}
      className="relative flex h-full w-full items-center justify-between"
    >
      {input ? (
        <div className="absolute left-0 top-1/2 -translate-x-1/2 -translate-y-1/2 [&_.pin-container]:h-5 [&_.pin-container>span]:hidden [&_.pin-circle]:m-0 [&_.pin-circle]:h-5 [&_.pin-circle]:w-5">
          <Pin
            {...input}
            graphPath={graphPath}
            groupId={groupId}
            isActive={activePinId === input.id}
            onPinClick={onPinClick}
            onPinPointerDown={onPinPointerDown}
          />
        </div>
      ) : null}
      <div
        data-reroute-grip
        data-reroute-kind={kind}
        className={gripClassName(kind)}
        style={{ width: REROUTE_GRIP_SIZE_PX, height: REROUTE_GRIP_SIZE_PX }}
      />
      {output ? (
        <div className="absolute right-0 top-1/2 translate-x-1/2 -translate-y-1/2 [&_.pin-container]:h-5 [&_.pin-container>span]:hidden [&_.pin-circle]:m-0 [&_.pin-circle]:h-5 [&_.pin-circle]:w-5">
          <Pin
            {...output}
            graphPath={graphPath}
            groupId={groupId}
            isActive={activePinId === output.id}
            onPinClick={onPinClick}
            onPinPointerDown={onPinPointerDown}
          />
        </div>
      ) : null}
    </div>
  );
}
