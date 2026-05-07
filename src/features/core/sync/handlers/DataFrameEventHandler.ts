// src/features/core/sync/handlers/DataFrameEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import {
    DataFrameCreatedPayload,
    DataFrameDeletedPayload,
    DataFrameSchemaUpdatedPayload,
    EventCallbacks,
} from '../types';
import { useDatabaseStore } from '@/features/core/dataStore';

export class DataFrameCreatedHandler extends BaseEventHandler<DataFrameCreatedPayload> {
    eventType = 'DataFrameCreated';
    
    handle(payload: DataFrameCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('DataFrame created:', payload.id);
        
        useDatabaseStore.getState().addDatabase(payload.id, payload.data as Record<string, unknown>);
        
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
 * SQL / Excel 等非真·lazy 数据源在项目打开后由后端后台物化。
 * 收到事件即把 schema 字段补到对应 db record 上，并清掉 loading 标记。
 */
export class DataFrameSchemaUpdatedHandler extends BaseEventHandler<DataFrameSchemaUpdatedPayload> {
    eventType = 'DataFrameSchemaUpdated';

    handle(payload: DataFrameSchemaUpdatedPayload): void {
        this.log('DataFrame schema updated:', payload.id, payload.error ? `error=${payload.error}` : `rows=${payload.rowCount}`);

        const patch: Record<string, unknown> = {
            loading: false,
        };

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
