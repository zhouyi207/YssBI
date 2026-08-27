import type { PlatformOutcome } from './platformTypes';

export interface WebviewWindowAdapter {
  readonly open: (label: string, url: string) => Promise<PlatformOutcome<void>>;
  readonly exists: (label: string) => Promise<PlatformOutcome<boolean>>;
}
