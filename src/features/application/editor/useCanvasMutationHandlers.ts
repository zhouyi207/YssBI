import { useMemo } from "react";
import { i18n } from "@/app/i18n";
import type { CanvasInteractionHandlers, CanvasMutationOutcome } from "@/features/core/canvas";
import { executeSafeGraphDraftEditOutcome } from "@/features/application/graphDraft/safeGraphDraftEdit";
import { graphDraftErrorMessageKey } from "@/features/application/graphDraft/graphDraftError";
import { logger } from "@/features/application/observability/appLogger";
import { insertRerouteAtConnection } from "./edgeOperations";
import { ensureGraphDraftPortRegistered } from "@/features/application/graphDraft/registerGraphDraftPort";

function toCanvasMutationOutcome(
  outcome: Awaited<ReturnType<typeof executeSafeGraphDraftEditOutcome>>,
): CanvasMutationOutcome {
  if (outcome !== false && outcome.status === "applied") return { status: "applied" };
  if (outcome === false) return { status: "failed" };
  const code = outcome.status === "rejected" ? outcome.code : null;
  const key = code ? graphDraftErrorMessageKey(code) : null;
  return key ? { status: "failed", message: i18n.t(key) } : { status: "failed" };
}

export function createCanvasMutationHandlers(): CanvasInteractionHandlers {
  ensureGraphDraftPortRegistered();
  return {
    async submitConnection({ graphPath, intent, sourcePinId, targetPinId }) {
      const outcome =
        intent === "connect"
          ? await executeSafeGraphDraftEditOutcome(graphPath, "Canvas connect", "ConnectPins", {
              pinA: sourcePinId,
              pinB: targetPinId,
            })
          : await executeSafeGraphDraftEditOutcome(
              graphPath,
              "Canvas move connections",
              "MoveConnections",
              { sourcePinId, targetPinId },
            );
      return toCanvasMutationOutcome(outcome);
    },
    async disconnectPort(graphPath, pinId) {
      return toCanvasMutationOutcome(
        await executeSafeGraphDraftEditOutcome(graphPath, "Alt disconnect port", "DisconnectPort", {
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
