import type { DatabaseReadSnapshot } from './read';

export interface DatabasePublicationCapability {
  readonly publishDatabase: (snapshot: DatabaseReadSnapshot) => void;
  readonly removeDatabase: (id: string) => void;
}
