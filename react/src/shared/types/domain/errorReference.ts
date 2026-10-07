/** Stable error identity shared by business projections and IPC adapters. */
export interface ErrorReference {
  readonly code: string;
  readonly incidentId: string | null;
}
