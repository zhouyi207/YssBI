// src/features/core/sync/handlers/DataFrameEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { DataFrameCreatedPayload, DataFrameDeletedPayload, EventCallbacks } from '../types';
import { useProjectStore } from '@/features/core/project';

export class DataFrameCreatedHandler extends BaseEventHandler<DataFrameCreatedPayload> {
    eventType = 'DataFrameCreated';
    
    handle(payload: DataFrameCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('DataFrame created:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.addDatabase(payload.id, payload.data);
        
        callbacks?.onDataFrameCreated?.(payload.id, payload.data);
    }
}

export class DataFrameDeletedHandler extends BaseEventHandler<DataFrameDeletedPayload> {
    eventType = 'DataFrameDeleted';
    
    handle(payload: DataFrameDeletedPayload, callbacks?: EventCallbacks): void {
        this.log('DataFrame deleted:', payload.id);
        
        const projectStore = useProjectStore.getState();
        projectStore.deleteDatabase(payload.id);
        
        callbacks?.onDataFrameDeleted?.(payload.id);
    }
}
