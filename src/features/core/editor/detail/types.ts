import type { LogMessage } from '@/shared/types/ui';

export type DetailTarget =
  | { kind: 'node'; id: string; graphId: string }
  | { kind: 'variable'; id: string }
  | { kind: 'data'; id: string }
  | { kind: 'log' }
  | { kind: 'event'; id: string }
  | { kind: 'function'; id: string }
  | { kind: 'worksheet'; id: string };

/** Explicit user selection for the Detail panel — no derived priority chain. */
export type DetailFocus = DetailTarget;

export interface DetailTargetInput {
  detailFocus: DetailFocus | null;
  selectedLog: LogMessage | null;
}
