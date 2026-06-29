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

export {
  connectionItemToConnectionData,
  connectionDataToItem,
  validateGraphDTO,
  cloneDTO,
  mergeProjectData,
} from '@/shared/types/dto/graphConverters';

export {
  buildCreateNodeRequest,
  type CreateNodeRequest,
} from './internalNodes';
