import type { NodeDefinition } from './node';
import type { TypeSystemSnapshot } from './typeSystem';

export interface EditorSchema {
  nodeDefinitions: NodeDefinition[];
  typeSystem: TypeSystemSnapshot;
}
