import type { ConnectionIntent } from "@/shared/types/domain/connectionCandidates";

export interface CanvasMutationOutcome {
  status: "applied" | "failed";
}

export interface CanvasGestureLease {
  isCurrent(): boolean;
  finish(): void;
}

export interface CanvasConnectionMutation {
  graphPath: string;
  intent: ConnectionIntent;
  sourcePinId: string;
  targetPinId: string;
}

export interface CanvasRerouteMutation {
  graphPath: string;
  connectionId: string;
  position: Readonly<{ x: number; y: number }>;
}

export interface CanvasMutationFailure {
  graphPath: string;
  intent: ConnectionIntent | "moveNodes" | "disconnectPort";
}

export interface CanvasInteractionHandlers {
  submitNodePositions(
    graphPath: string,
    positions: Array<{ nodeId: string; position: { x: number; y: number } }>,
  ): Promise<CanvasMutationOutcome>;
  submitConnection(mutation: CanvasConnectionMutation): Promise<CanvasMutationOutcome>;
  disconnectPort(graphPath: string, pinId: string): Promise<CanvasMutationOutcome>;
  insertRerouteAtConnection(mutation: CanvasRerouteMutation): Promise<CanvasMutationOutcome>;
  reportMutationFailure(failure: CanvasMutationFailure): void;
}
