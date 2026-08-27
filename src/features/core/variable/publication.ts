import type { VariableReadSnapshot } from './read';

export interface VariablePublicationCapability {
  readonly publishVariable: (snapshot: VariableReadSnapshot) => void;
  readonly removeVariable: (id: string) => void;
}
