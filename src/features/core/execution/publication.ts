import type { ExecutionReadSnapshot } from './read';

export interface ExecutionPublicationCapability {
  readonly publishExecution: (snapshot: ExecutionReadSnapshot) => void;
  readonly resetExecution: () => void;
}
