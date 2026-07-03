/** Backend-authoritative undo patch for structural restore (delete, disconnect, paste redo). */

export interface ConnectionRebuildDTO {
  fromPin: string;
  toPin: string;
}

/** Full pin instance as returned by the Rust backend (aligned with disk format). */
export interface SubgraphPinInstance {
  id: string;
  definition: Record<string, unknown>;
  userValue?: unknown;
}

export interface NodeSubgraphDTO {
  id: string;
  nodeType: string;
  position: { x: number; y: number };
  typeVarMap?: Record<string, unknown>;
  instanceParams?: Record<string, unknown>;
  pins: SubgraphPinInstance[];
}

export interface GraphUndoPatch {
  nodes: NodeSubgraphDTO[];
  /** Dynamic neighbors frozen at mutation time (delete / disconnect). */
  neighborNodes?: NodeSubgraphDTO[];
  connections: ConnectionRebuildDTO[];
}
