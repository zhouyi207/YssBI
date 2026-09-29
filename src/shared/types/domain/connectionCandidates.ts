import type { GraphEditVersionDto } from "./editorMutation";
import type { PortAddressDto } from "./editorProjection";

export type ConnectionIntent = "connect" | "moveConnections";
export type ConnectionDecision =
  | { kind: "append" }
  | { kind: "replace"; displacedConnectionIds: readonly string[] }
  | { kind: "invalid"; reason: string };

export interface ConnectionCandidatesRequest {
  projectInstanceId: string;
  graphPath: string;
  version: GraphEditVersionDto;
  sourcePort: PortAddressDto;
  intent: ConnectionIntent;
}

export interface ConnectionCandidates extends ConnectionCandidatesRequest {
  semanticInputHash: string;
  candidates: Array<{ port: PortAddressDto; decision: ConnectionDecision }>;
}
