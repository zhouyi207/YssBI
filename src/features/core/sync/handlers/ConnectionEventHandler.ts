import { BaseEventHandler } from './BaseEventHandler';
import {
    ConnectionCreatedPayload,
    ConnectionDeletedPayload,
    ConnectionsBatchDeletedPayload,
    EventCallbacks,
} from '../types';
import { useGraphDataStore } from '@/features/core/dataStore';

export class ConnectionCreatedHandler extends BaseEventHandler<ConnectionCreatedPayload> {
    eventType = 'ConnectionCreated';

    handle(payload: ConnectionCreatedPayload, _callbacks?: EventCallbacks): void {
        this.log('Connection created:', payload.fromPin, '->', payload.toPin, 'in graph:', payload.graphId);
        useGraphDataStore.getState().connect(payload.fromPin, payload.toPin);
    }
}

export class ConnectionDeletedHandler extends BaseEventHandler<ConnectionDeletedPayload> {
    eventType = 'ConnectionDeleted';

    handle(payload: ConnectionDeletedPayload, _callbacks?: EventCallbacks): void {
        this.log('Connection deleted:', payload.fromPin, '->', payload.toPin, 'in graph:', payload.graphId);
        const connectionId = `${payload.fromPin}->${payload.toPin}`;
        useGraphDataStore.getState().disconnect(connectionId);
    }
}

export class ConnectionsBatchDeletedHandler extends BaseEventHandler<ConnectionsBatchDeletedPayload> {
    eventType = 'ConnectionsBatchDeleted';

    handle(payload: ConnectionsBatchDeletedPayload, _callbacks?: EventCallbacks): void {
        this.log('Connections batch deleted:', payload.removedConnections.length, 'in graph:', payload.graphId);
        const store = useGraphDataStore.getState();
        // 使用 batchDisconnect 避免多次 set
        const connectionIds = payload.removedConnections.map(
            ([from, to]) => `${from}->${to}`
        );
        store.batchDisconnect(connectionIds);
    }
}
