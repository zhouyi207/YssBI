import type { ResourceReadSnapshot } from './read';

export interface ResourcePublicationCapability {
  readonly publishResource: (snapshot: ResourceReadSnapshot) => void;
  readonly removeResource: (resourcePath: string) => void;
}
