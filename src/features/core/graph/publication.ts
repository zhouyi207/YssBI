import type { DeepReadonly } from '@/features/core/projection/deepReadonly';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import type { GraphEntityBucket } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphMetaStore, type GraphMeta } from '@/features/core/dataStore/graphMetaStore';
import { getGraphSnapshot, type GraphProjectionSnapshot } from './read';

export interface OptimisticOperationKey {
  readonly projectInstanceId: string;
  readonly resourceKey: string;
  readonly operationId: string;
  readonly fromRevision: number;
}

export type GraphOverlay = Readonly<Record<string, unknown>>;

export interface GraphCommittedDelta {
  readonly graphPath: string;
  readonly graphEntities: DeepReadonly<GraphEntityBucket>;
  readonly graphMeta?: DeepReadonly<GraphMeta>;
}

export interface GraphProjectionPublication {
  replaceSnapshot(snapshot: DeepReadonly<GraphProjectionSnapshot>): void;
  applyCommittedDelta(delta: DeepReadonly<GraphCommittedDelta>): void;
  beginOptimisticOverlay(
    key: OptimisticOperationKey,
    overlay: DeepReadonly<GraphOverlay>,
  ): void;
  getOptimisticOverlay(key: OptimisticOperationKey): DeepReadonly<GraphOverlay> | undefined;
  settleOptimisticOverlay(key: OptimisticOperationKey): 'settled' | 'missing';
  rejectOptimisticOverlay(key: OptimisticOperationKey): 'rejected' | 'missing';
  invalidateOptimisticOverlay(key: OptimisticOperationKey): 'invalidated' | 'missing';
  clearForProject(projectInstanceId: string | null): void;
}

export function optimisticOperationKey(key: OptimisticOperationKey): string {
  return JSON.stringify([
    key.projectInstanceId,
    key.resourceKey,
    key.operationId,
    key.fromRevision,
  ]);
}

function cloneValue<T>(value: T): T {
  if (Array.isArray(value)) return value.map(cloneValue) as T;
  if (value === null || typeof value !== 'object') return value;
  if (value instanceof Date) return new Date(value.getTime()) as T;
  if (value instanceof Map) {
    return new Map(
      [...value.entries()].map(([key, nested]) => [cloneValue(key), cloneValue(nested)]),
    ) as T;
  }
  if (value instanceof Set) return new Set([...value].map(cloneValue)) as T;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .map(([key, nested]) => [key, cloneValue(nested)]),
  ) as T;
}

function freezeOverlayValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return Object.freeze(value.map(freezeOverlayValue));
  }
  if (value === null || typeof value !== 'object') return value;
  return Object.freeze(Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .map(([key, nested]) => [key, freezeOverlayValue(nested)]),
  ));
}

function freezeOverlay(
  overlay: DeepReadonly<GraphOverlay>,
): DeepReadonly<GraphOverlay> {
  return freezeOverlayValue(overlay) as DeepReadonly<GraphOverlay>;
}

export function createGraphProjectionPublication(): GraphProjectionPublication {
  const overlays = new Map<string, DeepReadonly<GraphOverlay>>();

  const removeOverlay = (
    key: OptimisticOperationKey,
  ): boolean => overlays.delete(optimisticOperationKey(key));

  return {
    replaceSnapshot: (snapshot) => {
      useGraphDataStore.setState({
        graphEntities: cloneValue(snapshot.graphEntities) as Record<string, GraphEntityBucket>,
      });
      useGraphMetaStore.setState({
        graphs: cloneValue(snapshot.graphMeta) as Record<string, GraphMeta>,
      });
    },

    applyCommittedDelta: (delta) => {
      const current = getGraphSnapshot();
      const graphEntities = {
        ...current.graphEntities,
        [delta.graphPath]: delta.graphEntities,
      };
      const graphMeta = delta.graphMeta === undefined
        ? current.graphMeta
        : { ...current.graphMeta, [delta.graphPath]: delta.graphMeta };
      useGraphDataStore.setState({
        graphEntities: cloneValue(graphEntities) as Record<string, GraphEntityBucket>,
      });
      useGraphMetaStore.setState({
        graphs: cloneValue(graphMeta) as Record<string, GraphMeta>,
      });
    },

    beginOptimisticOverlay: (key, overlay) => {
      overlays.set(optimisticOperationKey(key), freezeOverlay(overlay));
    },

    getOptimisticOverlay: (key) => overlays.get(optimisticOperationKey(key)),

    settleOptimisticOverlay: (key) =>
      removeOverlay(key) ? 'settled' : 'missing',

    rejectOptimisticOverlay: (key) =>
      removeOverlay(key) ? 'rejected' : 'missing',

    invalidateOptimisticOverlay: (key) =>
      removeOverlay(key) ? 'invalidated' : 'missing',

    clearForProject: (projectInstanceId) => {
      if (projectInstanceId === null) {
        overlays.clear();
      } else {
        for (const id of overlays.keys()) {
          const parsed = JSON.parse(id) as [string, string, string, number];
          if (parsed[0] === projectInstanceId) overlays.delete(id);
        }
      }
      useGraphDataStore.setState({ graphEntities: {} });
      useGraphMetaStore.setState({ graphs: {} });
    },
  };
}
