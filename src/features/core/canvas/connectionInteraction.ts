import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { resolveConnectionCompatibility } from "@/features/domain/editorProjection/connectionRules";

export const CONNECTION_SNAP_RADIUS_PX = 18;

export type ConnectionInvalidReason =
  | "same-port"
  | "same-node"
  | "same-direction"
  | "kind-mismatch"
  | "type-mismatch"
  | "orphan"
  | "capacity";

export type ConnectionFeedback =
  | { kind: "append" }
  | { kind: "replace"; displacedConnectionIds: readonly string[] }
  | { kind: "invalid"; reason: ConnectionInvalidReason };

export interface ConnectionCandidate {
  pin: PinData;
  center: { x: number; y: number };
  connectionIds: string[];
}

export interface ConnectionTargetResult {
  hoveredTarget: PinData | null;
  snappedTarget: PinData | null;
  snappedCenter: { x: number; y: number } | null;
  feedback: ConnectionFeedback | null;
}

const REASONS = {
  samePort: "same-port",
  sameNode: "same-node",
  directionMismatch: "same-direction",
  kindMismatch: "kind-mismatch",
  typeMismatch: "type-mismatch",
  orphan: "orphan",
  capacityReached: "capacity",
} as const;

export function resolveConnectionFeedback(
  source: PinData,
  target: PinData,
  connectionIds: Record<string, string[]> = {},
): ConnectionFeedback {
  const compatibility = resolveConnectionCompatibility(source, target);
  if (compatibility.kind === "invalid") {
    return { kind: "invalid", reason: REASONS[compatibility.reason] };
  }
  if (compatibility.kind === "replace") {
    return {
      kind: "replace",
      displacedConnectionIds: [
        ...new Set([...(connectionIds[source.id] ?? []), ...(connectionIds[target.id] ?? [])]),
      ].sort(),
    };
  }
  return compatibility;
}

export function resolveConnectionTarget(input: {
  source: PinData;
  sourceConnectionIds: string[];
  pointer: { x: number; y: number };
  candidates: ConnectionCandidate[];
}): ConnectionTargetResult {
  const candidates = input.candidates
    .filter((candidate) => candidate.pin.id !== input.source.id)
    .map((candidate) => ({
      ...candidate,
      distanceSquared:
        (candidate.center.x - input.pointer.x) ** 2 + (candidate.center.y - input.pointer.y) ** 2,
      feedback: resolveConnectionFeedback(input.source, candidate.pin, {
        [input.source.id]: input.sourceConnectionIds,
        [candidate.pin.id]: candidate.connectionIds,
      }),
    }))
    .filter((candidate) => candidate.distanceSquared <= CONNECTION_SNAP_RADIUS_PX ** 2)
    .sort((a, b) => a.distanceSquared - b.distanceSquared || a.pin.id.localeCompare(b.pin.id));
  const hovered = candidates[0] ?? null;
  const snapped = candidates.find((candidate) => candidate.feedback.kind !== "invalid") ?? null;
  return {
    hoveredTarget: hovered?.pin ?? null,
    snappedTarget: snapped?.pin ?? null,
    snappedCenter: snapped?.center ?? null,
    feedback: snapped?.feedback ?? hovered?.feedback ?? null,
  };
}
