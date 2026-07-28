export interface PendingMutationRecord {
  operationId: string;
  graphPath: string;
  baseRevision: number;
}

const pendingMutations = new Map<string, PendingMutationRecord>();
const invalidatedMutationIds = new Set<string>();

export function registerPendingMutation(record: PendingMutationRecord): void {
  if (pendingMutations.has(record.operationId)) {
    throw new Error(`mutation operation '${record.operationId}' is already pending`);
  }
  invalidatedMutationIds.delete(record.operationId);
  pendingMutations.set(record.operationId, record);
}

export function getPendingMutation(operationId: string): PendingMutationRecord | undefined {
  return pendingMutations.get(operationId);
}

export function completePendingMutation(operationId: string): PendingMutationRecord | undefined {
  const record = pendingMutations.get(operationId);
  pendingMutations.delete(operationId);
  return record;
}

export function invalidatePendingMutation(operationId: string): PendingMutationRecord | undefined {
  const record = completePendingMutation(operationId);
  invalidatedMutationIds.add(operationId);
  return record;
}

export function isInvalidatedMutation(operationId: string): boolean {
  return invalidatedMutationIds.has(operationId);
}

export function invalidatePendingMutationsForGraph(graphPath: string): void {
  for (const [operationId, record] of pendingMutations) {
    if (record.graphPath === graphPath) pendingMutations.delete(operationId);
  }
}

export function resetPendingMutations(): void {
  pendingMutations.clear();
  invalidatedMutationIds.clear();
}
