import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphSubgraphService } from "@/services/nodeSystem/graphSubgraphService";
import type { ClipboardSubgraphDto } from "@/shared/types/domain/clipboardSubgraph";

export async function exportEditorSubgraph(input: {
  graphPath: string;
  nodeIds: string[];
}): Promise<ClipboardSubgraphDto> {
  const identity = captureProjectIdentity();
  const snapshot = await GraphSubgraphService.exportSubgraph(
    identity.projectInstanceId,
    input.graphPath,
    input.nodeIds,
  );
  assertCurrentProjectIdentity(identity);
  return snapshot;
}
