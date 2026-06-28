// src/features/core/sync/handlers/GraphEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { GraphCreatedPayload, GraphUpdatedPayload, GraphDeletedPayload, GraphCreatedFailedPayload, EventCallbacks } from '../types';
import { useGraphMetaStore, useGraphDataStore } from '@/features/core/dataStore';
import { markGraphTabDirty } from '@/features/core/layout/tabDirty';
import { updateOpenResourceLabels, useResourceStore } from '@/features/core/resource';
import type { Graph } from '@/shared/types/domain';

type GraphWithMeta = Graph & { entryNodeId?: string };

function syncGraphResource(payload: GraphUpdatedPayload, kind: 'event' | 'function'): void {
    const name = payload.data.name;
    if (name === undefined) return;
    useResourceStore.getState().patchResource({ id: payload.id, kind }, { name });
    updateOpenResourceLabels({ id: payload.id, kind }, name);
}

// ==================== Event Handlers ====================

export class EventCreatedHandler extends BaseEventHandler<GraphCreatedPayload> {
    eventType = 'EventCreated';
    
    handle(payload: GraphCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Event created:', payload.id);
        
        const g = payload.data as GraphWithMeta;
        useGraphMetaStore.getState().addGraph({ id: g.id, name: g.name, type: 'event', entryNodeId: g.entryNodeId });
        useResourceStore.getState().upsertResource({
            id: g.id,
            kind: 'event',
            name: g.name,
            uri: `yssbi://graph/event/${g.id}`,
            exists: true,
            loaded: false,
            hasDirtyDocument: false,
            hasStaleDocument: false,
            hasConflictDocument: false,
        });
        
        callbacks?.onEventCreated?.(payload.id, payload.data);
    }
}

export class EventUpdatedHandler extends BaseEventHandler<GraphUpdatedPayload> {
    eventType = 'EventUpdated';
    
    handle(payload: GraphUpdatedPayload): void {
        this.log('Event updated:', payload.id);
        
        const meta = useGraphMetaStore.getState().graphs[payload.id];
        if (meta && payload.data.name !== undefined) useGraphMetaStore.getState().updateGraph(payload.id, { name: payload.data.name });
        syncGraphResource(payload, 'event');
        if (payload.data.nodes) useGraphDataStore.getState().addGraphFromData(payload.id, { ...meta, ...payload.data, nodes: payload.data.nodes } as unknown as import('@/shared/types/store/graph').GraphDataLike);
        markGraphTabDirty(payload.id);
    }
}

export class EventDeletedHandler extends BaseEventHandler<GraphDeletedPayload> {
    eventType = 'EventDeleted';
    
    handle(payload: GraphDeletedPayload): void {
        this.log('Event deleted:', payload.id);
        
        useGraphDataStore.getState().clearGraph(payload.id);
        useGraphMetaStore.getState().deleteGraph(payload.id);
        useResourceStore.getState().removeResource({ id: payload.id, kind: 'event' });
    }
}

export class EventCreatedFailedHandler extends BaseEventHandler<GraphCreatedFailedPayload> {
    eventType = 'EventCreatedFailed';
    
    handle(payload: GraphCreatedFailedPayload, callbacks?: EventCallbacks): void {
        this.error('Event creation failed:', payload.name, payload.error);
        
        callbacks?.onEventCreatedFailed?.(payload.name, payload.error);
    }
}

// ==================== Function Handlers ====================

export class FunctionCreatedHandler extends BaseEventHandler<GraphCreatedPayload> {
    eventType = 'FunctionCreated';
    
    handle(payload: GraphCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Function created:', payload.id);
        
        const g = payload.data as GraphWithMeta;
        useGraphMetaStore.getState().addGraph({ id: g.id, name: g.name, type: 'function', entryNodeId: g.entryNodeId });
        useResourceStore.getState().upsertResource({
            id: g.id,
            kind: 'function',
            name: g.name,
            uri: `yssbi://graph/function/${g.id}`,
            exists: true,
            loaded: false,
            hasDirtyDocument: false,
            hasStaleDocument: false,
            hasConflictDocument: false,
        });
        
        callbacks?.onFunctionCreated?.(payload.id, payload.data);
    }
}

export class FunctionUpdatedHandler extends BaseEventHandler<GraphUpdatedPayload> {
    eventType = 'FunctionUpdated';
    
    handle(payload: GraphUpdatedPayload): void {
        this.log('Function updated:', payload.id);
        
        const meta = useGraphMetaStore.getState().graphs[payload.id];
        if (meta && payload.data.name !== undefined) useGraphMetaStore.getState().updateGraph(payload.id, { name: payload.data.name });
        syncGraphResource(payload, 'function');
        if (payload.data.nodes) useGraphDataStore.getState().addGraphFromData(payload.id, { ...meta, ...payload.data, nodes: payload.data.nodes } as unknown as import('@/shared/types/store/graph').GraphDataLike);
        markGraphTabDirty(payload.id);
    }
}

export class FunctionDeletedHandler extends BaseEventHandler<GraphDeletedPayload> {
    eventType = 'FunctionDeleted';
    
    handle(payload: GraphDeletedPayload): void {
        this.log('Function deleted:', payload.id);
        
        useGraphDataStore.getState().clearGraph(payload.id);
        useGraphMetaStore.getState().deleteGraph(payload.id);
        useResourceStore.getState().removeResource({ id: payload.id, kind: 'function' });
    }
}

export class FunctionCreatedFailedHandler extends BaseEventHandler<GraphCreatedFailedPayload> {
    eventType = 'FunctionCreatedFailed';
    
    handle(payload: GraphCreatedFailedPayload, callbacks?: EventCallbacks): void {
        this.error('Function creation failed:', payload.name, payload.error);
        
        callbacks?.onFunctionCreatedFailed?.(payload.name, payload.error);
    }
}

