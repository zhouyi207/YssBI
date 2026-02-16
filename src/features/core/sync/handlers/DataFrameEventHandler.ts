// src/features/core/sync/handlers/DataFrameEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { DataFrameCreatedPayload, DataFrameDeletedPayload, EventCallbacks } from '../types';
import { useDatabaseStore } from '../../dataStore';

export class DataFrameCreatedHandler extends BaseEventHandler<DataFrameCreatedPayload> {
    eventType = 'DataFrameCreated';
    
    handle(payload: DataFrameCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('DataFrame created:', payload.id);
        
        const databaseStore = useDatabaseStore.getState();
        databaseStore.addDatabase(payload.id, payload.data);
        
        callbacks?.onDataFrameCreated?.(payload.id, payload.data);
    }
}

export class DataFrameDeletedHandler extends BaseEventHandler<DataFrameDeletedPayload> {
    eventType = 'DataFrameDeleted';
    
    handle(payload: DataFrameDeletedPayload, callbacks?: EventCallbacks): void {
        this.log('DataFrame deleted:', payload.id);
        
        const databaseStore = useDatabaseStore.getState();
        databaseStore.deleteDatabase(payload.id);
        
        callbacks?.onDataFrameDeleted?.(payload.id);
    }
}
