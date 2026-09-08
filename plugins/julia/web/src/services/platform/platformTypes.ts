export interface PlatformFailure {
  code: string;
  incidentId?: string | null;
  operation?: string;
}
export type PlatformOutcome<T> = { ok: true; value: T } | { ok: false; failure: PlatformFailure };
