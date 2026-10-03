import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphSubgraphService } from "@/services/nodeSystem/graphSubgraphService";
import type { ClipboardSubgraphDto } from "@/shared/types/domain/clipboardSubgraph";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { isGraphSaving } from "@/features/core/graph/read";

export async function exportEditorSubgraph(input: {
  graphPath: string;
  nodeIds: string[];
}): Promise<ClipboardSubgraphDto> {
  if (isGraphSaving(input.graphPath)) {
    throw new Error(`Graph draft '${input.graphPath}' is being saved`);
  }
  const identity = captureProjectIdentity();
  const version = useResourceStore.getState().sessions[input.graphPath]?.version;
  if (!version) throw new Error(`Graph draft '${input.graphPath}' is not loaded`);
  const snapshot = await GraphSubgraphService.exportSubgraph(
    identity.projectInstanceId,
    input.graphPath,
    version,
    input.nodeIds,
  );
  assertCurrentProjectIdentity(identity);
  return snapshot;
}
