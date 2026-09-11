export type CanvasMutationOutcome = { status: "applied" } | { status: "failed"; message?: string };

export type CanvasConnectionIntent = "connect" | "moveConnections";

export interface CanvasGestureLease {
  isCurrent(): boolean;
  finish(): void;
}

export interface CanvasConnectionMutation {
  graphPath: string;
  intent: CanvasConnectionIntent;
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
  intent: CanvasConnectionIntent | "moveNodes" | "disconnectPort";
  message?: string;
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
