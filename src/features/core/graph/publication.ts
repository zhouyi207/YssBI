import type { GraphReadSnapshot } from './read';

export interface GraphPublicationCapability {
  readonly publishGraph: (snapshot: GraphReadSnapshot) => void;
  readonly removeGraph: (graphPath: string) => void;
}
