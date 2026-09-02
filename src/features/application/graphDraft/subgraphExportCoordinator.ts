import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphSubgraphService } from "@/services/nodeSystem/graphSubgraphService";
import type { ClipboardSubgraphDto } from "@/shared/types/domain/clipboardSubgraph";
import { getGraphDraftDocument, isGraphDraftSaving } from "@/features/core/graphDraft";

export async function exportEditorSubgraph(input: {
  graphPath: string;
  nodeIds: string[];
}): Promise<ClipboardSubgraphDto> {
  if (isGraphDraftSaving(input.graphPath)) {
    throw new Error(`Graph draft '${input.graphPath}' is being saved`);
  }
  const identity = captureProjectIdentity();
  const document = getGraphDraftDocument(input.graphPath);
  if (!document) throw new Error(`Graph draft '${input.graphPath}' is not loaded`);
  const snapshot = await GraphSubgraphService.exportSubgraph(
    identity.projectInstanceId,
    input.graphPath,
    document,
    input.nodeIds,
  );
  assertCurrentProjectIdentity(identity);
  return snapshot;
}
