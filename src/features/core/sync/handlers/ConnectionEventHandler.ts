import { BaseEventHandler } from './BaseEventHandler';
import {
    ConnectionCreatedPayload,
    ConnectionDeletedPayload,
    ConnectionsBatchDeletedPayload,
    EventCallbacks,
} from '../types';
import { useGraphDataStore } from '@/features/core/dataStore';
import { markGraphTabDirty } from '@/features/core/layout/tabDirty';

export class ConnectionCreatedHandler extends BaseEventHandler<ConnectionCreatedPayload> {
    eventType = 'ConnectionCreated';

    handle(payload: ConnectionCreatedPayload, _callbacks?: EventCallbacks): void {
        this.log('Connection created:', payload.fromPin, '->', payload.toPin, 'in graph:', payload.graphId);
        useGraphDataStore.getState().connect(payload.fromPin, payload.toPin);
        markGraphTabDirty(payload.graphId);
    }
}

export class ConnectionDeletedHandler extends BaseEventHandler<ConnectionDeletedPayload> {
    eventType = 'ConnectionDeleted';

    handle(payload: ConnectionDeletedPayload, _callbacks?: EventCallbacks): void {
        this.log('Connection deleted:', payload.fromPin, '->', payload.toPin, 'in graph:', payload.graphId);
        const connectionId = `${payload.fromPin}->${payload.toPin}`;
        useGraphDataStore.getState().disconnect(connectionId);
        markGraphTabDirty(payload.graphId);
    }
}

export class ConnectionsBatchDeletedHandler extends BaseEventHandler<ConnectionsBatchDeletedPayload> {
    eventType = 'ConnectionsBatchDeleted';

    handle(payload: ConnectionsBatchDeletedPayload, _callbacks?: EventCallbacks): void {
        this.log('Connections batch deleted:', payload.removedConnections.length, 'in graph:', payload.graphId);
        const store = useGraphDataStore.getState();

        const connectionIds = new Set<string>();
        for (const [fromPin, toPin] of payload.removedConnections) {
            for (const cid of store.pinConnections[fromPin] ?? []) {
                const conn = store.connections[cid];
                if (conn && (conn.to === toPin || conn.from === toPin)) {
                    connectionIds.add(cid);
                    break;
                }
            }
            for (const cid of store.pinConnections[toPin] ?? []) {
                const conn = store.connections[cid];
                if (conn && (conn.from === fromPin || conn.to === fromPin)) {
                    connectionIds.add(cid);
                    break;
                }
            }
        }
        if (connectionIds.size > 0) {
            store.batchDisconnect(Array.from(connectionIds));
            markGraphTabDirty(payload.graphId);
            return;
        }

        this.error(
            'Connection not found in store (frontend-backend out of sync):',
            'graphId=', payload.graphId,
            'removedConnections=', payload.removedConnections
        );
    }
}
