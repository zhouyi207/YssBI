// src/features/core/sync/handlers/DataFrameEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import {
    DataFrameCreatedPayload,
    DataFrameDeletedPayload,
    EventCallbacks,
} from '../types';
import { useDatabaseStore } from '@/features/core/dataStore';
import { normalizeDatabaseRecord } from '@/shared/types/dto/database';

export class DataFrameCreatedHandler extends BaseEventHandler<DataFrameCreatedPayload> {
    eventType = 'DataFrameCreated';
    
    handle(payload: DataFrameCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('DataFrame created:', payload.id);
        
        useDatabaseStore.getState().addDatabase(
            payload.id,
            normalizeDatabaseRecord(payload.id, payload.data),
        );
        
        callbacks?.onDataFrameCreated?.(payload.id, payload.data);
    }
}

export class DataFrameDeletedHandler extends BaseEventHandler<DataFrameDeletedPayload> {
    eventType = 'DataFrameDeleted';
    
    handle(payload: DataFrameDeletedPayload, callbacks?: EventCallbacks): void {
        this.log('DataFrame deleted:', payload.id);
        
        useDatabaseStore.getState().deleteDatabase(payload.id);
        
        callbacks?.onDataFrameDeleted?.(payload.id);
    }
}
