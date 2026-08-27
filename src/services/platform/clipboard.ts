import type { PlatformOutcome } from './platformTypes';

export interface ClipboardAdapter {
  readonly readText: () => Promise<PlatformOutcome<string>>;
  readonly writeText: (value: string) => Promise<PlatformOutcome<void>>;
}
