import type { ComputationSettingsMutationReceiptDto } from '@/shared/types/dto/projectComputationSettings';

export interface SyncApplicationEventPort {
  graphDelta(graphPath: string): void;
  computationSettingsChanged(receipt: ComputationSettingsMutationReceiptDto): void;
  resourceMutationCommitted(result: unknown): Promise<unknown>;
  applyProjectLifecycleReceipt(
    result: unknown,
    dependencies?: unknown,
  ): Promise<void>;
  clearProject(): void;
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
