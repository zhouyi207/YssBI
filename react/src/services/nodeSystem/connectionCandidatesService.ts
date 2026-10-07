import { invokeCommand } from "@/services/ipc";
import { isPortAddressDto } from "@/shared/types/dto/editorProjectionGuards";
import { portAddressKey } from "@/shared/types/domain/portAddressKey";
import type {
  ConnectionCandidates,
  ConnectionCandidatesRequest,
  ConnectionDecision,
} from "@/shared/types/domain/connectionCandidates";

function exact(value: unknown, keys: string[]): value is Record<string, unknown> {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
  );
}

function decision(value: unknown): value is ConnectionDecision {
  if (exact(value, ["kind"]) && value.kind === "append") return true;
  if (exact(value, ["kind", "reason"]) && value.kind === "invalid") {
    return typeof value.reason === "string" && value.reason.length > 0;
  }
  return (
    exact(value, ["kind", "displacedConnectionIds"]) &&
    value.kind === "replace" &&
    Array.isArray(value.displacedConnectionIds) &&
    value.displacedConnectionIds.length > 0 &&
    value.displacedConnectionIds.every((id) => typeof id === "string" && id.length > 0) &&
    new Set(value.displacedConnectionIds).size === value.displacedConnectionIds.length
  );
}

export async function getConnectionCandidates(
  request: ConnectionCandidatesRequest,
): Promise<ConnectionCandidates> {
  const response: unknown = await invokeCommand("get_connection_candidates", { request });
  if (
    !exact(response, [
      "projectInstanceId",
      "graphPath",
      "version",
      "semanticInputHash",
      "sourcePort",
      "intent",
      "candidates",
    ]) ||
    response.projectInstanceId !== request.projectInstanceId ||
    response.graphPath !== request.graphPath ||
    !exact(response.version, ["sessionId", "revision"]) ||
    response.version.sessionId !== request.version.sessionId ||
    response.version.revision !== request.version.revision ||
    typeof response.semanticInputHash !== "string" ||
    !/^[0-9a-f]{64}$/.test(response.semanticInputHash) ||
    !isPortAddressDto(response.sourcePort) ||
    portAddressKey(response.sourcePort) !== portAddressKey(request.sourcePort) ||
    response.intent !== request.intent ||
    !Array.isArray(response.candidates)
  )
    throw new Error("Invalid connection candidates response");
  const seen = new Set<string>();
  for (const candidate of response.candidates) {
    if (
      !exact(candidate, ["port", "decision"]) ||
      !isPortAddressDto(candidate.port) ||
      !decision(candidate.decision)
    ) {
      throw new Error("Invalid connection candidate");
    }
    const key = portAddressKey(candidate.port);
    if (seen.has(key)) throw new Error("Duplicate connection candidate");
    seen.add(key);
  }
  return response as unknown as ConnectionCandidates;
}
