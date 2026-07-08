import { BaseEventHandler } from './BaseEventHandler';
import type {
  ProjectIndexInvalidatedPayload,
  ResourceChangedPayload,
  ResourceDeletedPayload,
  GraphResourceMovedPayload,
} from '../types';
import {
  getDocumentState,
  normalizeBackendResourceMeta,
  updateOpenResourceLabels,
  useResourceStore,
} from '@/features/core/resource';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { migrateGraphResourcePath } from '@/features/application/editor/migrateGraphResourcePath';

let invalidationDebounceTimer: ReturnType<typeof setTimeout> | null = null;
let invalidationRefreshPromise: Promise<boolean> | null = null;

function scheduleResourceIndexRefresh(): Promise<boolean> {
  if (invalidationRefreshPromise) {
    return invalidationRefreshPromise;
  }

  invalidationRefreshPromise = new Promise((resolve) => {
    if (invalidationDebounceTimer) {
      clearTimeout(invalidationDebounceTimer);
    }

    invalidationDebounceTimer = setTimeout(() => {
      invalidationDebounceTimer = null;
      void useProjectIOStore
        .getState()
        .refreshResourceIndex()
        .then(resolve)
        .finally(() => {
          invalidationRefreshPromise = null;
        });
    }, 50);
  });

  return invalidationRefreshPromise;
}

export class ProjectIndexInvalidatedHandler extends BaseEventHandler<ProjectIndexInvalidatedPayload> {
  eventType = 'ProjectIndexInvalidated';

  handle(payload: ProjectIndexInvalidatedPayload): void {
    this.log('Project index invalidated:', payload.source, payload.version);
    void scheduleResourceIndexRefresh();
  }
}

export class ResourceChangedHandler extends BaseEventHandler<ResourceChangedPayload> {
  eventType = 'ResourceChanged';

  handle(payload: ResourceChangedPayload): void {
    this.log('Resource changed:', payload.kind, payload.id);
    const meta = normalizeBackendResourceMeta(payload.data);
    const doc =
      meta.kind === 'event' || meta.kind === 'function' || meta.kind === 'worksheet'
        ? getDocumentState({ id: meta.id, kind: meta.kind })
        : undefined;
    useResourceStore.getState().upsertResource({
      ...meta,
      hasDirtyDocument: doc?.dirty ?? meta.hasDirtyDocument,
      hasStaleDocument: doc?.stale ?? meta.hasStaleDocument,
      hasConflictDocument: doc?.conflict ?? meta.hasConflictDocument,
      loaded: doc?.loaded ?? meta.loaded,
    });
    if (meta.kind === 'event' || meta.kind === 'function') {
      updateOpenResourceLabels({ id: meta.id, kind: meta.kind }, meta.name);
    }
  }
}

export class ResourceDeletedHandler extends BaseEventHandler<ResourceDeletedPayload> {
  eventType = 'ResourceDeleted';

  handle(payload: ResourceDeletedPayload): void {
    this.log('Resource deleted:', payload.kind, payload.id);
    useResourceStore.getState().removeResource({ id: payload.id, kind: payload.kind });
  }
}

export class GraphResourceMovedHandler extends BaseEventHandler<GraphResourceMovedPayload> {
  eventType = 'GraphResourceMoved';

  handle(payload: GraphResourceMovedPayload): void {
    this.log('Graph resource moved:', payload.from, '->', payload.to);
    if (payload.kind !== 'event' && payload.kind !== 'function') return;
    migrateGraphResourcePath(payload.from, payload.to, payload.kind);
    void scheduleResourceIndexRefresh();
  }
}
