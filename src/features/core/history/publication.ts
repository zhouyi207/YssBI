import type { HistoryReadSnapshot } from './read';

export interface HistoryPublicationCapability {
  readonly publishHistory: (snapshot: HistoryReadSnapshot) => void;
}
