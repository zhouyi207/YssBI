export interface PendingMutationPort {
  graphPathFor(operationId: string): string | undefined;
}

let port: PendingMutationPort | null = null;

export function registerPendingMutationPort(next: PendingMutationPort): void {
  port = next;
}

export function resetPendingMutationPort(): void {
  port = null;
}

export function pendingMutationGraphPath(operationId: string): string | undefined {
  return port?.graphPathFor(operationId);
}
