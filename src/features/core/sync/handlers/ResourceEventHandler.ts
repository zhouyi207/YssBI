import { BaseEventHandler } from './BaseEventHandler';
import type { ResourceChangedPayload, ResourceDeletedPayload } from '../types';
import { useGraphMetaStore } from '@/features/core/dataStore';
import {
    markResourceExternalChanged,
    markResourceMissing,
    normalizeBackendResourceMeta,
    updateOpenResourceLabels,
    useResourceStore,
} from '@/features/core/resource';

export class ResourceChangedHandler extends BaseEventHandler<ResourceChangedPayload> {
    eventType = 'ResourceChanged';

    handle(payload: ResourceChangedPayload): void {
        this.log('Resource changed:', payload.kind, payload.id);
        const meta = normalizeBackendResourceMeta(payload.data);
        useResourceStore.getState().upsertResource(meta);
        if (payload.source === 'watcher') {
            markResourceExternalChanged({ id: meta.id, kind: meta.kind });
        }
        if (meta.kind === 'event' || meta.kind === 'function') {
            useGraphMetaStore.getState().updateGraph(meta.id, {
                name: meta.name,
                type: meta.kind,
                folderPath: meta.folderPath,
            });
            updateOpenResourceLabels({ id: meta.id, kind: meta.kind }, meta.name);
        }
    }
}

export class ResourceDeletedHandler extends BaseEventHandler<ResourceDeletedPayload> {
    eventType = 'ResourceDeleted';

    handle(payload: ResourceDeletedPayload): void {
        this.log('Resource deleted:', payload.kind, payload.id);
        if (payload.source === 'watcher') {
            markResourceMissing({ id: payload.id, kind: payload.kind });
            return;
        }
        useResourceStore.getState().removeResource({ id: payload.id, kind: payload.kind });
        if (payload.kind === 'event' || payload.kind === 'function') {
            useGraphMetaStore.getState().deleteGraph(payload.id);
        }
    }
}
