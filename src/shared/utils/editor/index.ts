// Connection helpers (pure functions, no external deps)
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
  areConnectionsValid,
} from './connections';

// DTO converters（已迁移至 @/shared/types/dto/graphConverters）
export {
  convertGraphFromDTO,
  convertGraphToDTO,
  convertGraphsFromDTO,
  convertGraphsToDTO,
  convertProjectDataFromDTO,
  convertProjectDataToDTO,
  validateGraphDTO,
  cloneDTO,
  mergeProjectData,
} from '@/shared/types/dto/graphConverters';

// Internal nodes (pure functions, no external deps)
export {
  buildCreateNodeRequest,
  type CreateNodeRequest,
  syncInternalNodePins,
  syncGraphInstanceNodes,
} from './internalNodes';
