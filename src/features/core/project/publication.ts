import type { ReadonlyProjectSnapshot } from "./read";

export interface ProjectPublicationCapability {
  readonly publishProjectSnapshot: (snapshot: ReadonlyProjectSnapshot) => void;
  readonly resetProjectSnapshot: () => void;
}
