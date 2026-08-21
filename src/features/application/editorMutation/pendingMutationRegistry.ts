export interface PendingMutationRecord {
  operationId: string;
  graphPath: string;
  baseRevision: number;
}

const pendingMutations = new Map<string, PendingMutationRecord>();
const invalidatedMutationIds = new Set<string>();
const settlementWaitersByGraph = new Map<string, Set<() => void>>();

export function hasPendingGraphMutations(graphPath: string): boolean {
  for (const record of pendingMutations.values()) {
    if (record.graphPath === graphPath) return true;
  }
  return false;
}

function resolveGraphSettlementWaiters(graphPath: string): void {
  if (hasPendingGraphMutations(graphPath)) return;
  const waiters = settlementWaitersByGraph.get(graphPath);
  if (!waiters) return;
  settlementWaitersByGraph.delete(graphPath);
  for (const resolve of waiters) resolve();
}

export function waitForPendingGraphMutations(graphPath: string): Promise<void> {
  if (!hasPendingGraphMutations(graphPath)) return Promise.resolve();
  return new Promise((resolve) => {
    const waiters = settlementWaitersByGraph.get(graphPath) ?? new Set();
    waiters.add(resolve);
    settlementWaitersByGraph.set(graphPath, waiters);
  });
}

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
  if (record) resolveGraphSettlementWaiters(record.graphPath);
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
  resolveGraphSettlementWaiters(graphPath);
}

export function resetPendingMutations(): void {
  pendingMutations.clear();
  invalidatedMutationIds.clear();
  for (const waiters of settlementWaitersByGraph.values()) {
    for (const resolve of waiters) resolve();
  }
  settlementWaitersByGraph.clear();
}
