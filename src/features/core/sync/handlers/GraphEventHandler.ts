// src/features/core/sync/handlers/GraphEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { GraphCreatedPayload, GraphUpdatedPayload, GraphDeletedPayload, GraphCreatedFailedPayload, EventCallbacks } from '../types';
import { syncFunctionSignatureFromGraph } from '@/features/application/graphDocument/functionSignatureSync';
import { shouldSuppressIncrementalPinUpdate } from '@/features/application/graphDocument/graphDocumentActions';
import { useGraphDataStore } from '@/features/core/dataStore';
import { markGraphTabDirty } from '@/features/core/layout/tabDirty';
import { updateOpenResourceLabels, useResourceStore, resourceKey } from '@/features/core/resource';
import type { Graph } from '@/shared/types/domain';
import type { ProjectResourceMeta } from '@/features/core/resource';
import type { GraphDataLike } from '@/shared/types/store/graph';
import { graphUpdatedPayloadToGraphDataLike } from '@/shared/types/dto/graphModel';

type GraphWithMeta = Graph & { entryNodeId?: string };

function getGraphResourceMeta(graphId: string, kind: 'event' | 'function') {
  return useResourceStore.getState().resources[resourceKey({ id: graphId, kind })];
}

function syncGraphResource(payload: GraphUpdatedPayload, kind: 'event' | 'function'): void {
    const name = payload.data.name;
    if (name === undefined) return;
    useResourceStore.getState().patchResource({ id: payload.id, kind }, { name });
    updateOpenResourceLabels({ id: payload.id, kind }, name);
}

function buildGraphUpdateData(
  payload: GraphUpdatedPayload,
  meta: ProjectResourceMeta,
  kind: 'event' | 'function',
): GraphDataLike {
  return graphUpdatedPayloadToGraphDataLike(payload.id, kind, meta.name, payload.data);
}

// ==================== Event Handlers ====================

export class EventCreatedHandler extends BaseEventHandler<GraphCreatedPayload> {
    eventType = 'EventCreated';
    
    handle(payload: GraphCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Event created:', payload.id);
        
        const g = payload.data as GraphWithMeta;
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
        
        const meta = getGraphResourceMeta(payload.id, 'event');
        syncGraphResource(payload, 'event');
        if (payload.data.nodes && meta) {
          useGraphDataStore.getState().addGraphFromData(
            payload.id,
            buildGraphUpdateData(payload, meta, 'event'),
          );
        }
        markGraphTabDirty(payload.id);
    }
}

export class EventDeletedHandler extends BaseEventHandler<GraphDeletedPayload> {
    eventType = 'EventDeleted';
    
    handle(payload: GraphDeletedPayload): void {
        this.log('Event deleted:', payload.id);
        
        useGraphDataStore.getState().clearGraph(payload.id);
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
        syncFunctionSignatureFromGraph(g);
        
        callbacks?.onFunctionCreated?.(payload.id, payload.data);
    }
}

export class FunctionUpdatedHandler extends BaseEventHandler<GraphUpdatedPayload> {
    eventType = 'FunctionUpdated';
    
    handle(payload: GraphUpdatedPayload): void {
        this.log('Function updated:', payload.id);
        
        const meta = getGraphResourceMeta(payload.id, 'function');
        syncGraphResource(payload, 'function');
        const name = payload.data.name ?? meta?.name;
        if (name) {
          syncFunctionSignatureFromGraph({
            id: payload.id,
            name,
            type: 'function',
            functionInputs: payload.data.functionInputs,
            functionOutputs: payload.data.functionOutputs,
          });
        }
        if (payload.data.nodes && meta && !shouldSuppressIncrementalPinUpdate(payload.id)) {
          useGraphDataStore.getState().addGraphFromData(
            payload.id,
            buildGraphUpdateData(payload, meta, 'function'),
          );
        }
        markGraphTabDirty(payload.id);
    }
}

export class FunctionDeletedHandler extends BaseEventHandler<GraphDeletedPayload> {
    eventType = 'FunctionDeleted';
    
    handle(payload: GraphDeletedPayload): void {
        this.log('Function deleted:', payload.id);
        
        useGraphDataStore.getState().clearGraph(payload.id);
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
