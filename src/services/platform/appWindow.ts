import type { CloseRequestDecision, PlatformOutcome } from './platformTypes';

export interface AppWindowAdapter {
  readonly label: () => Promise<PlatformOutcome<string>>;
  readonly show: () => Promise<PlatformOutcome<void>>;
  readonly close: () => Promise<PlatformOutcome<void>>;
  readonly decideClose: () => CloseRequestDecision;
}
