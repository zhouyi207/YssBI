// src/features/core/sync/handlers/GraphEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { GraphCreatedPayload, GraphUpdatedPayload, GraphDeletedPayload, GraphCreatedFailedPayload, EventCallbacks } from '../types';
import { useGraphMetaStore, useGraphDataStore } from '@/features/core/dataStore';
import type { Graph } from '@/shared/types/domain';

type GraphWithMeta = Graph & { entryNodeId?: string };

// ==================== Event Handlers ====================

export class EventCreatedHandler extends BaseEventHandler<GraphCreatedPayload> {
    eventType = 'EventCreated';
    
    handle(payload: GraphCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Event created:', payload.id);
        
        const g = payload.data as GraphWithMeta;
        useGraphMetaStore.getState().addGraph({ id: g.id, name: g.name, type: 'event', entryNodeId: g.entryNodeId });
        useGraphDataStore.getState().addGraphFromData(payload.id, {
          ...g,
          nodes: g.nodes ?? [],
          pins: g.pins ?? [],
          connections: g.connections ?? { connections: [] },
          canvas: g.canvas ?? { x: 0, y: 0, scale: 1 },
        } as unknown as import('@/shared/types/store/graph').GraphDataLike);
        
        callbacks?.onEventCreated?.(payload.id, payload.data);
    }
}

export class EventUpdatedHandler extends BaseEventHandler<GraphUpdatedPayload> {
    eventType = 'EventUpdated';
    
    handle(payload: GraphUpdatedPayload): void {
        this.log('Event updated:', payload.id);
        
        const meta = useGraphMetaStore.getState().graphs[payload.id];
        if (meta && payload.data.name !== undefined) useGraphMetaStore.getState().updateGraph(payload.id, { name: payload.data.name });
        if (payload.data.nodes) useGraphDataStore.getState().addGraphFromData(payload.id, { ...meta, ...payload.data, nodes: payload.data.nodes } as unknown as import('@/shared/types/store/graph').GraphDataLike);
    }
}

export class EventDeletedHandler extends BaseEventHandler<GraphDeletedPayload> {
    eventType = 'EventDeleted';
    
    handle(payload: GraphDeletedPayload): void {
        this.log('Event deleted:', payload.id);
        
        useGraphDataStore.getState().clearGraph(payload.id);
        useGraphMetaStore.getState().deleteGraph(payload.id);
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
        useGraphDataStore.getState().addGraphFromData(payload.id, {
          ...g,
          nodes: g.nodes ?? [],
          pins: g.pins ?? [],
          connections: g.connections ?? { connections: [] },
          canvas: g.canvas ?? { x: 0, y: 0, scale: 1 },
        } as unknown as import('@/shared/types/store/graph').GraphDataLike);
        
        callbacks?.onFunctionCreated?.(payload.id, payload.data);
    }
}

export class FunctionUpdatedHandler extends BaseEventHandler<GraphUpdatedPayload> {
    eventType = 'FunctionUpdated';
    
    handle(payload: GraphUpdatedPayload): void {
        this.log('Function updated:', payload.id);
        
        const meta = useGraphMetaStore.getState().graphs[payload.id];
        if (meta && payload.data.name !== undefined) useGraphMetaStore.getState().updateGraph(payload.id, { name: payload.data.name });
        if (payload.data.nodes) useGraphDataStore.getState().addGraphFromData(payload.id, { ...meta, ...payload.data, nodes: payload.data.nodes } as unknown as import('@/shared/types/store/graph').GraphDataLike);
    }
}

export class FunctionDeletedHandler extends BaseEventHandler<GraphDeletedPayload> {
    eventType = 'FunctionDeleted';
    
    handle(payload: GraphDeletedPayload): void {
        this.log('Function deleted:', payload.id);
        
        useGraphDataStore.getState().clearGraph(payload.id);
        useGraphMetaStore.getState().deleteGraph(payload.id);
    }
}

export class FunctionCreatedFailedHandler extends BaseEventHandler<GraphCreatedFailedPayload> {
    eventType = 'FunctionCreatedFailed';
    
    handle(payload: GraphCreatedFailedPayload, callbacks?: EventCallbacks): void {
        this.error('Function creation failed:', payload.name, payload.error);
        
        callbacks?.onFunctionCreatedFailed?.(payload.name, payload.error);
    }
}

