// src/features/core/sync/handlers/DataFrameEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import {
    DataFrameCreatedPayload,
    DataFrameDeletedPayload,
    DataFrameSchemaUpdatedPayload,
    EventCallbacks,
} from '../types';
import { useDatabaseStore } from '@/features/core/dataStore';
import { normalizeDatabaseRecord } from '@/shared/types/dto/database';
import type { DatabaseRecord } from '@/shared/types/dto/database';

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

/**
 * DuckDB 数据集 schema 更新（遗留事件；正常路径在 get_project_data 中同步返回 schema）。
 */
export class DataFrameSchemaUpdatedHandler extends BaseEventHandler<DataFrameSchemaUpdatedPayload> {
    eventType = 'DataFrameSchemaUpdated';

    handle(payload: DataFrameSchemaUpdatedPayload): void {
        this.log('DataFrame schema updated:', payload.id, payload.error ? `error=${payload.error}` : `rows=${payload.rowCount}`);

        const patch: Partial<DatabaseRecord> = {};

        if (payload.error) {
            patch.loadError = payload.error;
        } else {
            patch.columns = payload.columns;
            patch.rowCount = payload.rowCount;
            patch.columnCount = payload.columnCount;
            patch.loadError = undefined;
        }

        useDatabaseStore.getState().updateDatabase(payload.id, patch);
    }
}
