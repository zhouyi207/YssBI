import { ensureGraphDraftPortRegistered } from "@/features/application/graphDraft/registerGraphDraftPort";
import { executeSafeGraphDraftEditOutcome } from "@/features/application/graphDraft/safeGraphDraftEdit";
import type { GraphDraftCommandResult } from "@/features/core/history/types";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";

function isNonEmpty(value: string): boolean {
  return value.trim().length > 0;
}

export function addPortInstance(
  graphPath: string,
  nodeId: string,
  templateKey: string,
): Promise<GraphDraftCommandResult> {
  if (!isNonEmpty(nodeId) || !isNonEmpty(templateKey)) return Promise.resolve(false);
  ensureGraphDraftPortRegistered();
  return executeSafeGraphDraftEditOutcome(graphPath, "Add port instance", "AddPortInstance", {
    nodeId,
    templateKey,
  });
}

export function removePortInstance(
  graphPath: string,
  address: Extract<PortAddressDto, { kind: "instance" }>,
): Promise<GraphDraftCommandResult> {
  ensureGraphDraftPortRegistered();
  return executeSafeGraphDraftEditOutcome(graphPath, "Remove port instance", "RemovePortInstance", {
    address,
  });
}
