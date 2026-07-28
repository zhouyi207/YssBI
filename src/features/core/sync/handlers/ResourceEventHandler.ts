import { BaseEventHandler } from './BaseEventHandler';
import type {
  ProjectIndexInvalidatedPayload,
  ResourceChangedPayload,
} from '../types';
import {
  getDocumentState,
  notifyIndexInvalidated,
  normalizeBackendResourceMeta,
  useResourceStore,
} from '@/features/core/resource';

export class ProjectIndexInvalidatedHandler extends BaseEventHandler<ProjectIndexInvalidatedPayload> {
  eventType = 'ProjectIndexInvalidated';

  handle(payload: ProjectIndexInvalidatedPayload): void {
    this.log('Project index invalidated:', payload.source, payload.version);
    void notifyIndexInvalidated('watcher');
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
  }
}
