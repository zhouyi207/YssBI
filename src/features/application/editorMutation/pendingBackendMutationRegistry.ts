export interface PendingBackendMutationRecord {
  operationId: string;
  graphPath: string;
  baseRevision: number;
}

const pendingMutations = new Map<string, PendingBackendMutationRecord>();

export function registerPendingBackendMutation(record: PendingBackendMutationRecord): void {
  if (pendingMutations.has(record.operationId)) {
    throw new Error(`backend mutation operation '${record.operationId}' is already pending`);
  }
  pendingMutations.set(record.operationId, record);
}

export function getPendingBackendMutation(
  operationId: string,
): PendingBackendMutationRecord | undefined {
  return pendingMutations.get(operationId);
}

export function completePendingBackendMutation(
  operationId: string,
): PendingBackendMutationRecord | undefined {
  const record = pendingMutations.get(operationId);
  pendingMutations.delete(operationId);
  return record;
}

export function invalidatePendingBackendMutation(
  operationId: string,
): PendingBackendMutationRecord | undefined {
  return completePendingBackendMutation(operationId);
}

export function resetPendingBackendMutations(): void {
  pendingMutations.clear();
}
