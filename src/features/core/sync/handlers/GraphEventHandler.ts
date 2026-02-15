// src/features/core/sync/handlers/GraphEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { GraphCreatedPayload, GraphUpdatedPayload, GraphDeletedPayload, GraphCreatedFailedPayload, EventCallbacks } from '../types';
import { useProjectStore } from '@/features/core/project';

// ==================== Event Handlers ====================

export class EventCreatedHandler extends BaseEventHandler<GraphCreatedPayload> {
    eventType = 'EventCreated';
    
    handle(payload: GraphCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Event created:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.addGraph(payload.id, payload.data);
        
        callbacks?.onEventCreated?.(payload.id, payload.data);
    }
}

export class EventUpdatedHandler extends BaseEventHandler<GraphUpdatedPayload> {
    eventType = 'EventUpdated';
    
    handle(payload: GraphUpdatedPayload): void {
        this.log('Event updated:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.updateGraph(payload.id, payload.data);
    }
}

export class EventDeletedHandler extends BaseEventHandler<GraphDeletedPayload> {
    eventType = 'EventDeleted';
    
    handle(payload: GraphDeletedPayload): void {
        this.log('Event deleted:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.deleteGraph(payload.id);
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
        
        const projectStore = useProjectStore.getState();
        projectStore.addGraph(payload.id, payload.data);
        
        callbacks?.onFunctionCreated?.(payload.id, payload.data);
    }
}

export class FunctionUpdatedHandler extends BaseEventHandler<GraphUpdatedPayload> {
    eventType = 'FunctionUpdated';
    
    handle(payload: GraphUpdatedPayload): void {
        this.log('Function updated:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.updateGraph(payload.id, payload.data);
    }
}

export class FunctionDeletedHandler extends BaseEventHandler<GraphDeletedPayload> {
    eventType = 'FunctionDeleted';
    
    handle(payload: GraphDeletedPayload): void {
        this.log('Function deleted:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.deleteGraph(payload.id);
    }
}

export class FunctionCreatedFailedHandler extends BaseEventHandler<GraphCreatedFailedPayload> {
    eventType = 'FunctionCreatedFailed';
    
    handle(payload: GraphCreatedFailedPayload, callbacks?: EventCallbacks): void {
        this.error('Function creation failed:', payload.name, payload.error);
        
        callbacks?.onFunctionCreatedFailed?.(payload.name, payload.error);
    }
}

// ==================== Macro Handlers ====================

export class MacroCreatedHandler extends BaseEventHandler<GraphCreatedPayload> {
    eventType = 'MacroCreated';
    
    handle(payload: GraphCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Macro created:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.addGraph(payload.id, payload.data);
        
        callbacks?.onMacroCreated?.(payload.id, payload.data);
    }
}

export class MacroUpdatedHandler extends BaseEventHandler<GraphUpdatedPayload> {
    eventType = 'MacroUpdated';
    
    handle(payload: GraphUpdatedPayload): void {
        this.log('Macro updated:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.updateGraph(payload.id, payload.data);
    }
}

export class MacroDeletedHandler extends BaseEventHandler<GraphDeletedPayload> {
    eventType = 'MacroDeleted';
    
    handle(payload: GraphDeletedPayload): void {
        this.log('Macro deleted:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.deleteGraph(payload.id);
    }
}

export class MacroCreatedFailedHandler extends BaseEventHandler<GraphCreatedFailedPayload> {
    eventType = 'MacroCreatedFailed';
    
    handle(payload: GraphCreatedFailedPayload, callbacks?: EventCallbacks): void {
        this.error('Macro creation failed:', payload.name, payload.error);
        
        callbacks?.onMacroCreatedFailed?.(payload.name, payload.error);
    }
}
