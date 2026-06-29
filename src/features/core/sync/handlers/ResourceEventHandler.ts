import { BaseEventHandler } from './BaseEventHandler';
import type {
  ProjectIndexInvalidatedPayload,
  ResourceChangedPayload,
  ResourceDeletedPayload,
} from '../types';
import {
  normalizeBackendResourceMeta,
  updateOpenResourceLabels,
  useResourceStore,
} from '@/features/core/resource';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';

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
    useResourceStore.getState().upsertResource(meta);
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
