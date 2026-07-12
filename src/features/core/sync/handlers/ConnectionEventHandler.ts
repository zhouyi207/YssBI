import { BaseEventHandler } from './BaseEventHandler';
import {
    ConnectionCreatedPayload,
    ConnectionDeletedPayload,
    ConnectionsBatchDeletedPayload,
    ConnectionsBatchCreatedPayload,
    EventCallbacks,
} from '../types';
import { useGraphDataStore } from '@/features/core/dataStore';
import { markGraphTabDirty } from '@/features/core/layout/tabDirty';
import { isPending } from '../utils/echoSuppressor';
import { CONNECTION_ECHO_DOMAIN } from '@/features/core/history/commands/connectPins';

export class ConnectionCreatedHandler extends BaseEventHandler<ConnectionCreatedPayload> {
    eventType = 'ConnectionCreated';

    handle(payload: ConnectionCreatedPayload, _callbacks?: EventCallbacks): void {
        const connectionId = `${payload.fromPin}->${payload.toPin}`;
        // 自发起的连接已乐观写入 store，跳过回声避免二次 set
        if (isPending(CONNECTION_ECHO_DOMAIN, connectionId)) {
            markGraphTabDirty(payload.graphPath);
            return;
        }
        this.log('Connection created:', payload.fromPin, '->', payload.toPin, 'in graph:', payload.graphPath);
        useGraphDataStore.getState().connect(payload.fromPin, payload.toPin, payload.graphPath);
        markGraphTabDirty(payload.graphPath);
    }
}

export class ConnectionDeletedHandler extends BaseEventHandler<ConnectionDeletedPayload> {
    eventType = 'ConnectionDeleted';

    handle(payload: ConnectionDeletedPayload, _callbacks?: EventCallbacks): void {
        const connectionId = `${payload.fromPin}->${payload.toPin}`;
        // 自发起连接触发的自动断开已乐观处理，跳过回声
        if (isPending(CONNECTION_ECHO_DOMAIN, connectionId)) {
            markGraphTabDirty(payload.graphPath);
            return;
        }
        this.log('Connection deleted:', payload.fromPin, '->', payload.toPin, 'in graph:', payload.graphPath);
        useGraphDataStore.getState().disconnect(connectionId, payload.graphPath);
        markGraphTabDirty(payload.graphPath);
    }
}

export class ConnectionsBatchCreatedHandler extends BaseEventHandler<ConnectionsBatchCreatedPayload> {
    eventType = 'ConnectionsBatchCreated';

    handle(payload: ConnectionsBatchCreatedPayload, _callbacks?: EventCallbacks): void {
        this.log('Connections batch created:', payload.connections.length, 'in graph:', payload.graphPath);
        const pairs = payload.connections.map(([from, to]) => ({ from, to }));
        useGraphDataStore.getState().batchConnect(pairs, payload.graphPath);
        markGraphTabDirty(payload.graphPath);
    }
}

export class ConnectionsBatchDeletedHandler extends BaseEventHandler<ConnectionsBatchDeletedPayload> {
    eventType = 'ConnectionsBatchDeleted';

    handle(payload: ConnectionsBatchDeletedPayload, _callbacks?: EventCallbacks): void {
        this.log('Connections batch deleted:', payload.removedConnections.length, 'in graph:', payload.graphPath);
        const store = useGraphDataStore.getState();

        const connectionIds = new Set<string>();
        for (const [fromPin, toPin] of payload.removedConnections) {
            for (const cid of store.getGraphPinConnections(payload.graphPath, fromPin)) {
                const conn = store.getGraphConnection(payload.graphPath, cid);
                if (conn && (conn.to === toPin || conn.from === toPin)) {
                    connectionIds.add(cid);
                    break;
                }
            }
            for (const cid of store.getGraphPinConnections(payload.graphPath, toPin)) {
                const conn = store.getGraphConnection(payload.graphPath, cid);
                if (conn && (conn.from === fromPin || conn.to === fromPin)) {
                    connectionIds.add(cid);
                    break;
                }
            }
        }
        if (connectionIds.size > 0) {
            store.batchDisconnect(Array.from(connectionIds), payload.graphPath);
            markGraphTabDirty(payload.graphPath);
            return;
        }

        this.error(
            'Connection not found in store (frontend-backend out of sync):',
            'graphPath=', payload.graphPath,
            'removedConnections=', payload.removedConnections
        );
    }
}
