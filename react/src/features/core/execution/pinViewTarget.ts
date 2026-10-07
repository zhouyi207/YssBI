import { portAddressKey } from "@/features/domain/editorProjection";
import {
  outputPinRef,
  type InspectableResultRef,
} from "@/features/domain/result/inspectableResultRef";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { GraphEntityBucket } from "@/features/core/dataStore/graphEntityAccess";

export interface ResolvePinViewTargetParams {
  graphPath: string;
  address: PortAddressDto;
  direction: "input" | "output";
}

type PinConnectionIndex = DeepReadonly<Pick<GraphEntityBucket, "pinConnections" | "connections">>;

export function hasPinViewTarget(
  params: ResolvePinViewTargetParams,
  graph: PinConnectionIndex | undefined,
): boolean {
  if (params.direction === "output") return true;
  if (!graph) return false;
  const pinId = portAddressKey(params.address);
  const connectionIds = graph.pinConnections[pinId];
  if (!connectionIds) return false;
  for (const id of connectionIds) {
    if (graph.connections[id].to === pinId) return true;
  }
  return false;
}

export function inspectableRefsFromPinView(
  params: ResolvePinViewTargetParams,
  graph: PinConnectionIndex | undefined,
): InspectableResultRef[] {
  const { graphPath, address, direction } = params;
  if (direction === "output") return [outputPinRef(graphPath, address)];
  const refs: InspectableResultRef[] = [];
  if (!graph) return refs;
  const pinId = portAddressKey(address);
  const connectionIds = graph.pinConnections[pinId];
  if (!connectionIds) return refs;
  for (const id of connectionIds) {
    const connection = graph.connections[id];
    // Adjacency includes both endpoints, including diagnostic-blocked connections.
    if (connection.to === pinId) refs.push(outputPinRef(graphPath, connection.output));
  }
  return refs;
}
