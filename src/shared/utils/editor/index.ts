// Serialization utilities
export { serializeGraph, deserializeGraph } from './serialization';

// Node operations
export { 
  createNodeInBackend, 
  deleteNodeInBackend, 
  createNodeFromTemplate 
} from './nodeOperations';

// Internal nodes
export { 
  createInternalNode, 
  syncInternalNodePins, 
  syncGraphInstanceNodes 
} from './internalNodes';

// Connection helpers
export {
  findConnectionsByPin,
  findConnectionsByNode,
  findConnectionById,
  areConnected,
  findConnectionsFromPin,
  findConnectionsToPin,
  getTargetPins,
  getSourcePin,
  countConnectionsForPin,
  hasConnections,
  removeConnectionsForPin,
  removeConnectionsForNode,
  validateConnections,
  areConnectionsValid
} from './connections';

// Pin utilities
export { isSingleLinkPin, isCompatiblePins } from './pins';

// DTO converters
export {
  convertGraphFromDTO,
  convertGraphToDTO,
  convertGraphsFromDTO,
  convertGraphsToDTO,
  convertProjectDataFromDTO,
  convertProjectDataToDTO,
  applyConnectionsToPins,
  extractConnectionsFromPins,
  validateGraphDTO,
  cloneDTO,
  mergeProjectData,
} from './dtoConverters';

