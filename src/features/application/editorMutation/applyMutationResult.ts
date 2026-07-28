import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import type { AtomicProjectionApplyResult } from '@/features/core/dataStore/graphDataStore';
import type { GraphMutationResultDto } from '@/shared/types/dto/editorMutation';
import type { PendingMutationRecord } from './pendingMutationRegistry';

function invalidResult(message: string): never {
  throw new Error(`invalid mutation result: ${message}`);
}

export function validateMutationResult(
  pending: PendingMutationRecord,
  result: GraphMutationResultDto,
): void {
  const { delta, projectionReplacement } = result;
  if (delta.graphPath !== pending.graphPath) {
    invalidResult(`delta graph path '${delta.graphPath}' does not match '${pending.graphPath}'`);
  }
  if (delta.causedBy !== pending.operationId) {
    invalidResult(`operation correlation '${String(delta.causedBy)}' does not match '${pending.operationId}'`);
  }
  if (delta.fromRevision !== pending.baseRevision) {
    invalidResult(`from revision ${delta.fromRevision} does not match ${pending.baseRevision}`);
  }
  if (delta.toRevision !== delta.fromRevision + 1) {
    invalidResult(`revision ${delta.fromRevision} -> ${delta.toRevision} is not monotonic`);
  }
  if (projectionReplacement.graphPath !== delta.graphPath) {
    invalidResult(
      `replacement graph path '${projectionReplacement.graphPath}' does not match delta graph path '${delta.graphPath}'`,
    );
  }
  if (projectionReplacement.projection.sourceRevision !== delta.toRevision) {
    invalidResult(
      `replacement revision ${projectionReplacement.projection.sourceRevision} does not match committed revision ${delta.toRevision}`,
    );
  }
}

export function applyMutationResult(
  pending: PendingMutationRecord,
  result: GraphMutationResultDto,
): AtomicProjectionApplyResult {
  validateMutationResult(pending, result);
  return useGraphDataStore
    .getState()
    .replaceProjectionsAtomically([result.projectionReplacement]);
}
