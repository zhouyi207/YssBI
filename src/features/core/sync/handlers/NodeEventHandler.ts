// src/features/core/sync/handlers/NodeEventHandler.ts
// 按 README 范式：Handler 直接更新 Store，callbacks 仅用于可选 UI 扩展

import { BaseEventHandler } from './BaseEventHandler';
import { NodeCreatedPayload, NodeDeletedPayload, EventCallbacks } from '../types';
import { useGraphDataStore } from '@/features/core/dataStore';

export class NodeCreatedHandler extends BaseEventHandler<NodeCreatedPayload> {
    eventType = 'NodeCreated';

    handle(payload: NodeCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Node created:', payload.node_id, 'in graph:', payload.graph_id);

        const store = useGraphDataStore.getState();

        // 1. 添加节点到 Store
        const nodeData = {
            id: payload.node_id,
            ...payload.data,
            graphId: payload.graph_id,
            inputs: payload.data?.inputs ?? [],
            outputs: payload.data?.outputs ?? [],
        };
        store.addNode(payload.graph_id, nodeData as any);

        // 2. 添加 pins 到 Store（后端 NodeCreated 已包含 pins）
        payload.pins.forEach((pin: any) => {
            store.addPin(payload.node_id, pin);
        });

        // 3. 可选回调：UI 扩展
        callbacks?.onNodeCreated?.(payload.graph_id, payload.node_id, payload.data);
    }
}

export class NodeDeletedHandler extends BaseEventHandler<NodeDeletedPayload> {
    eventType = 'NodeDeleted';

    handle(payload: NodeDeletedPayload, callbacks?: EventCallbacks): void {
        this.log('Node deleted:', payload.node_id, 'from graph:', payload.graph_id);

        // 1. Handler 直接更新 Store
        useGraphDataStore.getState().deleteNode(payload.node_id);

        // 2. 可选回调：UI 扩展
        callbacks?.onNodeDeleted?.(payload.graph_id, payload.node_id);
    }
}
