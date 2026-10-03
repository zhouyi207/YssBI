import type { GraphConstantDto } from "@/shared/types/domain/editorMutation";
import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import type { GraphEntityBucket } from "@/features/core/dataStore/graphEntityAccess";
import type { GraphMeta } from "@/features/core/dataStore/graphMeta";
import type { GraphPath } from "@/shared/types/domain/ids";
import type { GraphRunIdentityDto } from "@/shared/types/domain/runEvent";

export interface GraphProjectionSnapshot {
  readonly graphEntities: DeepReadonly<Record<GraphPath, GraphEntityBucket>>;
  readonly graphMeta: DeepReadonly<Record<GraphPath, GraphMeta>>;
}

function buildSnapshot(): DeepReadonly<GraphProjectionSnapshot> {
  const { graphEntities, graphMeta } = useResourceStore.getState();
  return {
    graphEntities,
    graphMeta,
  };
}

const projection = createReadProjection(buildSnapshot, [useResourceStore]);
export const getGraphSnapshot = projection.getSnapshot;
export function useGraphRead<T>(
  selector: (snapshot: DeepReadonly<GraphProjectionSnapshot>) => T,
): T {
  return useReadProjection(projection, selector);
}

export function getGraphConstants(graphPath: string): Record<string, GraphConstantDto> | null {
  return useResourceStore.getState().sessions[graphPath]?.constants ?? null;
}
export function isGraphSaving(graphPath: string): boolean {
  return useResourceStore.getState().sessions[graphPath]?.saving === true;
}
export function isGraphModified(graphPath: string): boolean {
  return useResourceStore.getState().sessions[graphPath]?.saveDirty === true;
}

export function isCurrentGraphRun(run: GraphRunIdentityDto): boolean {
  const { sessions, resultStates } = useResourceStore.getState();
  const session = sessions[run.graphPath];
  // Public execution also covers graphs without a loaded editor.
  return (
    !session ||
    (session.semanticInputHash === run.semanticInputHash &&
      resultStates[run.graphPath]?.executionSessionId === run.executionSessionId)
  );
}

export function isGraphRunResultPending(run: GraphRunIdentityDto, resultRevision: string): boolean {
  const summary = useResourceStore.getState().resultStates[run.graphPath];
  return (
    !summary ||
    summary.executionSessionId !== run.executionSessionId ||
    summary.semanticInputHash !== run.semanticInputHash ||
    BigInt(summary.revision) < BigInt(resultRevision)
  );
}
