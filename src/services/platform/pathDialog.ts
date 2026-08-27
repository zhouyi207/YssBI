import type { PlatformOutcome } from './platformTypes';

export interface PathDialogAdapter {
  readonly open: () => Promise<PlatformOutcome<string | null>>;
  readonly save: () => Promise<PlatformOutcome<string | null>>;
}
