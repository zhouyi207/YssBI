import { useMemo } from "react";
import type { CanvasInteractionHandlers, CanvasMutationOutcome } from "@/features/core/canvas";
import { executeGraphEdit } from "@/features/application/graphEditing";
import { logger } from "@/features/application/observability/appLogger";
import { insertRerouteAtConnection } from "./edgeOperations";

function toCanvasMutationOutcome(
  outcome: Awaited<ReturnType<typeof executeGraphEdit>>,
): CanvasMutationOutcome {
  return { status: outcome.status === "applied" ? "applied" : "failed" };
}

export function createCanvasMutationHandlers(): CanvasInteractionHandlers {
  return {
    async submitNodePositions(graphPath, positions) {
      return toCanvasMutationOutcome(await executeGraphEdit(graphPath, "MoveNodes", { positions }));
    },
    async submitConnection({ graphPath, intent, sourcePinId, targetPinId }) {
      const outcome =
        intent === "connect"
          ? await executeGraphEdit(graphPath, "ConnectPins", {
              pinA: sourcePinId,
              pinB: targetPinId,
            })
          : await executeGraphEdit(graphPath, "MoveConnections", { sourcePinId, targetPinId });
      return toCanvasMutationOutcome(outcome);
    },
    async disconnectPort(graphPath, pinId) {
      return toCanvasMutationOutcome(
        await executeGraphEdit(graphPath, "DisconnectPort", {
          pinId,
        }),
      );
    },
    async insertRerouteAtConnection({ graphPath, connectionId, position }) {
      return toCanvasMutationOutcome(
        await insertRerouteAtConnection(graphPath, connectionId, position),
      );
    },
    reportMutationFailure({ graphPath, intent }) {
      logger.graph.warn(
        `Graph mutation failed graphPath=${graphPath} intent=${intent}`,
        "CanvasInteraction",
      );
    },
  };
}

export function useCanvasMutationHandlers(): CanvasInteractionHandlers {
  return useMemo(createCanvasMutationHandlers, []);
}
