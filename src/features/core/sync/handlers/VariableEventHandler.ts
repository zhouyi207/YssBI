// src/features/core/sync/handlers/VariableEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { VariableCreatedPayload, VariableUpdatedPayload, VariableDeletedPayload, EventCallbacks } from '../types';
import { useProjectStore } from '@/features/core/project';

export class VariableCreatedHandler extends BaseEventHandler<VariableCreatedPayload> {
    eventType = 'GlobalVariableCreated';
    
    handle(payload: VariableCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Variable created:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.addVariable(payload.id, payload.data);
        
        callbacks?.onVariableCreated?.(payload.id, payload.data);
    }
}

export class VariableUpdatedHandler extends BaseEventHandler<VariableUpdatedPayload> {
    eventType = 'GlobalVariableUpdated';
    
    handle(payload: VariableUpdatedPayload, callbacks?: EventCallbacks): void {
        this.log('Variable updated:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.updateVariable(payload.id, payload.data);
        
        callbacks?.onVariableUpdated?.(payload.id, payload.data);
    }
}

export class VariableDeletedHandler extends BaseEventHandler<VariableDeletedPayload> {
    eventType = 'GlobalVariableDeleted';
    
    handle(payload: VariableDeletedPayload, callbacks?: EventCallbacks): void {
        this.log('Variable deleted:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.deleteVariable(payload.id);
        
        callbacks?.onVariableDeleted?.(payload.id);
    }
}
