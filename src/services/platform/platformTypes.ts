export type PlatformFailureCode = 'unavailable' | 'rejected' | 'closed';

export interface PlatformFailure {
  readonly code: PlatformFailureCode;
}

export type PlatformOutcome<T> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly failure: PlatformFailure };

export type CloseRequestDecision = 'allow' | 'prevent';
