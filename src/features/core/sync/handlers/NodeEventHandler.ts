// src/features/core/sync/handlers/NodeEventHandler.ts
// 按 README 范式：Handler 直接更新 Store，callbacks 仅用于可选 UI 扩展

import { BaseEventHandler } from './BaseEventHandler';
import { NodeCreatedPayload, NodeDeletedPayload, NodePositionsUpdatedPayload, EventCallbacks } from '../types';
import { useGraphDataStore } from '@/features/core/dataStore';
import type { NodeData, PinData } from '@/shared/types';

export class NodeCreatedHandler extends BaseEventHandler<NodeCreatedPayload> {
    eventType = 'NodeCreated';

    handle(payload: NodeCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Node created:', payload.nodeId, 'in graph:', payload.graphId);

        const store = useGraphDataStore.getState();
        const d = payload.data;

        // 1. 添加节点到 Store（NodeInstanceDTO -> NodeData）
        const nodeData: NodeData = {
            id: payload.nodeId,
            graphId: payload.graphId,
            nodeType: d.nodeType,
            category: d.category ?? [],
            title: d.title ?? '',
            inputs: d.inputs ?? [],
            outputs: d.outputs ?? [],
            uiStyle: d.uiStyle ?? 'default',
            description: d.description,
            position: d.position ?? { x: 0, y: 0 },
        };
        store.addNode(payload.graphId, nodeData);

        // 2. 添加 pins 到 Store（后端 NodeCreated 已包含 pins）
        payload.pins.forEach((pin) => {
            store.addPin(payload.nodeId, pin as PinData);
        });

        // 3. 可选回调：UI 扩展
        callbacks?.onNodeCreated?.(payload.graphId, payload.nodeId, payload.data);
    }
}

export class NodeDeletedHandler extends BaseEventHandler<NodeDeletedPayload> {
    eventType = 'NodeDeleted';

    handle(payload: NodeDeletedPayload, callbacks?: EventCallbacks): void {
        this.log('Node deleted:', payload.nodeId, 'from graph:', payload.graphId);

        // 1. Handler 直接更新 Store
        useGraphDataStore.getState().deleteNode(payload.nodeId);

        // 2. 可选回调：UI 扩展
        callbacks?.onNodeDeleted?.(payload.graphId, payload.nodeId);
    }
}

export class NodePositionsUpdatedHandler extends BaseEventHandler<NodePositionsUpdatedPayload> {
    eventType = 'NodePositionsUpdated';

    handle(payload: NodePositionsUpdatedPayload, _callbacks?: EventCallbacks): void {
        this.log('Node positions updated:', payload.graphId, payload.updates.length, 'nodes');

        const updates = payload.updates.map(([nodeId, x, y]) => ({ nodeId, x, y }));
        useGraphDataStore.getState().batchUpdateNodePositions(updates);
    }
}
