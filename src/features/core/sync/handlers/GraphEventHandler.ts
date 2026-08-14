// src/features/core/sync/handlers/GraphEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { GraphUpdatedPayload, GraphDeletedPayload } from '../types';
import { syncApplicationEventPort } from '../applicationEventPort';
import { useGraphDataStore, useGraphMetaStore } from '@/features/core/dataStore';
import { markGraphTabDirty } from '@/features/core/layout/tabDirty';

import {
  lookupGraphResource,
  useResourceStore,
} from '@/features/core/resource';

function getGraphResourceMeta(graphPath: string, kind: 'event' | 'function') {
  return lookupGraphResource(useResourceStore.getState().resources, graphPath, kind);
}

function syncGraphResource(payload: GraphUpdatedPayload, kind: 'event' | 'function'): void {
    const name = payload.data.name;
    if (name === undefined) return;
    useResourceStore.getState().patchResource({ id: payload.path, kind }, { name });
}

// ==================== Event Handlers ====================

export class EventUpdatedHandler extends BaseEventHandler<GraphUpdatedPayload> {
    eventType = 'EventUpdated';
    
    handle(payload: GraphUpdatedPayload): void {
        this.log('Event updated:', payload.path);
        
        syncGraphResource(payload, 'event');
        syncApplicationEventPort().eventUpdated(payload.path);
        markGraphTabDirty(payload.path);
    }
}

export class EventDeletedHandler extends BaseEventHandler<GraphDeletedPayload> {
    eventType = 'EventDeleted';
    
    handle(payload: GraphDeletedPayload): void {
        this.log('Event deleted:', payload.path);
        
        useGraphDataStore.getState().clearGraph(payload.path);
        useGraphMetaStore.getState().deleteGraph(payload.path);
        useResourceStore.getState().removeResource({ id: payload.path, kind: 'event' });
    }
}

// ==================== Function Handlers ====================

export class FunctionUpdatedHandler extends BaseEventHandler<GraphUpdatedPayload> {
    eventType = 'FunctionUpdated';
    
    handle(payload: GraphUpdatedPayload): void {
        this.log('Function updated:', payload.path);
        
        const meta = getGraphResourceMeta(payload.path, 'function');
        syncGraphResource(payload, 'function');
        const name = payload.data.name ?? meta?.name;
        if (name) {
          syncApplicationEventPort().functionUpdated({
            path: payload.path,
            name,
            type: 'function',
            functionInputs: payload.data.functionInputs,
            functionOutputs: payload.data.functionOutputs,
          });
        }

        syncApplicationEventPort().eventUpdated(payload.path);
        markGraphTabDirty(payload.path);
    }
}

export class FunctionDeletedHandler extends BaseEventHandler<GraphDeletedPayload> {
    eventType = 'FunctionDeleted';
    
    handle(payload: GraphDeletedPayload): void {
        this.log('Function deleted:', payload.path);
        
        useGraphDataStore.getState().clearGraph(payload.path);
        useGraphMetaStore.getState().deleteGraph(payload.path);
        useResourceStore.getState().removeResource({ id: payload.path, kind: 'function' });
    }
}
