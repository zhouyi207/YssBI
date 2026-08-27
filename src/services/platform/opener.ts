import type { PlatformOutcome } from './platformTypes';

export interface ExternalOpener {
  readonly open: (target: string) => Promise<PlatformOutcome<void>>;
}
