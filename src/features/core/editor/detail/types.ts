import type { LogMessage } from '@/shared/types/ui';

export type DetailTarget =
  | { kind: 'node'; id: string; graphPath: string }
  | { kind: 'nodeDefinition'; nodeType: string }
  | { kind: 'variable'; id: string }
  | { kind: 'data'; id: string }
  | { kind: 'log' }
  | { kind: 'event'; path: string }
  | { kind: 'function'; path: string }
  | { kind: 'worksheet'; id: string };

/** Explicit user selection for the Detail panel — no derived priority chain. */
export type DetailFocus = DetailTarget;

export interface DetailTargetInput {
  detailFocus: DetailFocus | null;
  selectedLog: LogMessage | null;
}
