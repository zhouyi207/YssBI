// src/features/core/sync/handlers/GraphEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { GraphCreatedPayload, GraphUpdatedPayload, GraphDeletedPayload, GraphCreatedFailedPayload, EventCallbacks } from '../types';
import { syncFunctionSignatureFromGraph } from '@/features/application/graphDocument/functionSignatureSync';
import { shouldSuppressGraphRefreshEcho } from '@/features/application/graphDocument/graphRefreshEchoGuard';
import { useGraphDataStore, useGraphMetaStore } from '@/features/core/dataStore';
import { markGraphTabDirty } from '@/features/core/layout/tabDirty';
import {
  buildGraphResourceMeta,
  lookupGraphResource,
  markResourceLoaded,
  useResourceStore,
} from '@/features/core/resource';
import type { Graph } from '@/shared/types/domain';
import type { ProjectResourceMeta } from '@/features/core/resource';
import type { GraphDataLike } from '@/shared/types/store/graph';
import { graphUpdatedPayloadToGraphDataLike } from '@/shared/types/dto/graphModel';

type GraphWithMeta = Graph & { entryNodeId?: string };

function getGraphResourceMeta(graphPath: string, kind: 'event' | 'function') {
  return lookupGraphResource(useResourceStore.getState().resources, graphPath, kind);
}

function syncGraphResource(payload: GraphUpdatedPayload, kind: 'event' | 'function'): void {
    const name = payload.data.name;
    if (name === undefined) return;
    useResourceStore.getState().patchResource({ id: payload.path, kind }, { name });
}

function buildGraphUpdateData(
  payload: GraphUpdatedPayload,
  meta: ProjectResourceMeta,
  kind: 'event' | 'function',
): GraphDataLike {
  return graphUpdatedPayloadToGraphDataLike(payload.path, kind, meta.name, payload.data);
}

// ==================== Event Handlers ====================

export class EventCreatedHandler extends BaseEventHandler<GraphCreatedPayload> {
    eventType = 'EventCreated';
    
    handle(payload: GraphCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Event created:', payload.path);
        
        const g = payload.data as GraphWithMeta;
        useResourceStore.getState().upsertResource(
            buildGraphResourceMeta('event', payload.path, g.name),
        );
        useGraphDataStore.getState().addGraphFromData(
            payload.path,
            graphUpdatedPayloadToGraphDataLike(payload.path, 'event', g.name, g),
        );
        markResourceLoaded({ id: payload.path, kind: 'event' }, true);
        
        callbacks?.onEventCreated?.(payload.path, payload.data);
    }
}

export class EventUpdatedHandler extends BaseEventHandler<GraphUpdatedPayload> {
    eventType = 'EventUpdated';
    
    handle(payload: GraphUpdatedPayload): void {
        if (shouldSuppressGraphRefreshEcho(payload.path)) {
            this.log('Event updated (suppressed — invoke refresh authoritative):', payload.path);
            return;
        }

        this.log('Event updated:', payload.path);
        
        const meta = getGraphResourceMeta(payload.path, 'event');
        syncGraphResource(payload, 'event');
        if (payload.data.nodes && meta) {
          useGraphDataStore.getState().addGraphFromData(
            payload.path,
            buildGraphUpdateData(payload, meta, 'event'),
          );
        }
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
        this.log('Function created:', payload.path);
        
        const g = payload.data as GraphWithMeta;
        useResourceStore.getState().upsertResource(
            buildGraphResourceMeta('function', payload.path, g.name),
        );
        useGraphDataStore.getState().addGraphFromData(
            payload.path,
            graphUpdatedPayloadToGraphDataLike(payload.path, 'function', g.name, g),
        );
        markResourceLoaded({ id: payload.path, kind: 'function' }, true);
        syncFunctionSignatureFromGraph(g);
        
        callbacks?.onFunctionCreated?.(payload.path, payload.data);
    }
}

export class FunctionUpdatedHandler extends BaseEventHandler<GraphUpdatedPayload> {
    eventType = 'FunctionUpdated';
    
    handle(payload: GraphUpdatedPayload): void {
        if (shouldSuppressGraphRefreshEcho(payload.path)) {
            this.log('Function updated (suppressed — invoke refresh authoritative):', payload.path);
            return;
        }

        this.log('Function updated:', payload.path);
        
        const meta = getGraphResourceMeta(payload.path, 'function');
        syncGraphResource(payload, 'function');
        const name = payload.data.name ?? meta?.name;
        if (name) {
          syncFunctionSignatureFromGraph({
            path: payload.path,
            name,
            type: 'function',
            functionInputs: payload.data.functionInputs,
            functionOutputs: payload.data.functionOutputs,
          });
        }
        if (payload.data.nodes && meta) {
          useGraphDataStore.getState().addGraphFromData(
            payload.path,
            buildGraphUpdateData(payload, meta, 'function'),
          );
        }
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

export class FunctionCreatedFailedHandler extends BaseEventHandler<GraphCreatedFailedPayload> {
    eventType = 'FunctionCreatedFailed';
    
    handle(payload: GraphCreatedFailedPayload, callbacks?: EventCallbacks): void {
        this.error('Function creation failed:', payload.name, payload.error);
        
        callbacks?.onFunctionCreatedFailed?.(payload.name, payload.error);
    }
}
