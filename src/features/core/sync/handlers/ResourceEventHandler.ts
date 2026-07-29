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
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/services/project/projectIdentity';

function isCurrentProjectEvent(projectInstanceId: string): boolean {
  try {
    const identity = captureProjectIdentity();
    return identity.projectInstanceId === projectInstanceId
      && isCurrentProjectIdentity(identity);
  } catch {
    return false;
  }
}

export class ProjectIndexInvalidatedHandler extends BaseEventHandler<ProjectIndexInvalidatedPayload> {
  eventType = 'ProjectIndexInvalidated';

  handle(payload: ProjectIndexInvalidatedPayload): void {
    if (!isCurrentProjectEvent(payload.projectInstanceId)) return;
    this.log('Project index invalidated:', payload.source, payload.version);
    void notifyIndexInvalidated('watcher');
  }
}

export class ResourceChangedHandler extends BaseEventHandler<ResourceChangedPayload> {
  eventType = 'ResourceChanged';

  handle(payload: ResourceChangedPayload): void {
    if (!isCurrentProjectEvent(payload.projectInstanceId)) return;
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
