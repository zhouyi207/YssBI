export interface SyncApplicationEventPort {
  eventUpdated(graphPath: string): void;
  functionUpdated(payload: unknown): void;
  variablesChanged(): void;
  graphDelta(graphPath: string): void;
  resourceMutationCommitted(result: unknown): Promise<unknown>;
  applyProjectLifecycleReceipt(
    result: unknown,
    onProjectCleared?: () => void,
    dependencies?: unknown,
  ): Promise<void>;
  clearProject(onProjectCleared?: () => void): void;
}

let port: SyncApplicationEventPort | null = null;

export function registerSyncApplicationEventPort(next: SyncApplicationEventPort): void {
  port = next;
}

export function resetSyncApplicationEventPort(): void {
  port = null;
}

export function syncApplicationEventPort(): SyncApplicationEventPort {
  if (!port) throw new Error('Sync application event port is not registered');
  return port;
}
