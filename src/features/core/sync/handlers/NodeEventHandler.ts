// src/features/core/sync/handlers/NodeEventHandler.ts
// 按 README 范式：Handler 直接更新 Store，callbacks 仅用于可选 UI 扩展

import { BaseEventHandler } from './BaseEventHandler';
import { NodeCreatedPayload, NodesBatchCreatedPayload, NodeDeletedPayload, NodesBatchDeletedPayload, NodePositionsUpdatedPayload, NodePinsUpdatedPayload, PinTypesInferredPayload, EventCallbacks } from '../types';
import { useGraphDataStore } from '@/features/core/dataStore';
import { useNodeRegistryStore } from '@/features/core/nodeRegister/useNodeRegistryStore';
import type { NodeData, PinData } from '@/shared/types';
import type { NodeInstanceDTO } from '@/shared/types/dto';

function resolveNodeTitle(dto: NodeInstanceDTO): string {
    const raw = dto.title ?? '';
    const nodeType = dto.nodeType ?? '';
    if (raw && raw !== nodeType) return raw;
    const def = useNodeRegistryStore.getState().getDefinition(nodeType);
    return def?.name ?? raw ?? nodeType;
}

export class NodeCreatedHandler extends BaseEventHandler<NodeCreatedPayload> {
    eventType = 'NodeCreated';

    handle(payload: NodeCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Node created:', payload.nodeId, 'in graph:', payload.graphId);

        const store = useGraphDataStore.getState();
        store.addNode(payload.graphId, dtoToNodeData(payload.graphId, payload.nodeId, payload.data));
        payload.pins.forEach((pin) => {
            store.addPin(payload.nodeId, pin as PinData);
        });
        callbacks?.onNodeCreated?.(payload.graphId, payload.nodeId, payload.data);
    }
}

function dtoToNodeData(graphId: string, nodeId: string, d: NodeInstanceDTO): NodeData {
    return {
        id: nodeId,
        graphId,
        nodeType: d.nodeType,
        category: d.category ?? [],
        title: resolveNodeTitle(d),
        inputs: d.inputs ?? [],
        outputs: d.outputs ?? [],
        uiStyle: d.uiStyle ?? 'default',
        description: d.description,
        position: d.position ?? { x: 0, y: 0 },
        variableId: d.variableId,
        variableName: d.variableName,
        variableType: d.variableType,
        subGraphId: d.subGraphId,
        dataframeId: d.dataframeId,
        columnName: d.columnName,
        columnType: d.columnType,
    };
}

export class NodesBatchCreatedHandler extends BaseEventHandler<NodesBatchCreatedPayload> {
    eventType = 'NodesBatchCreated';

    handle(payload: NodesBatchCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Batch nodes created:', payload.nodes.length, 'in graph:', payload.graphId);

        const store = useGraphDataStore.getState();

        const items = payload.nodes.map(([nodeId, data, pins]) => ({
            node: dtoToNodeData(payload.graphId, nodeId, data),
            pins: pins as PinData[],
        }));

        store.batchAddNodesAndPins(payload.graphId, items);

        if (callbacks?.onNodeCreated) {
            for (const [nodeId, data] of payload.nodes) {
                callbacks.onNodeCreated(payload.graphId, nodeId, data);
            }
        }
    }
}

export class NodeDeletedHandler extends BaseEventHandler<NodeDeletedPayload> {
    eventType = 'NodeDeleted';

    handle(payload: NodeDeletedPayload, callbacks?: EventCallbacks): void {
        this.log('Node deleted:', payload.nodeId, 'from graph:', payload.graphId);
        useGraphDataStore.getState().deleteNode(payload.nodeId);
        callbacks?.onNodeDeleted?.(payload.graphId, payload.nodeId);
    }
}

export class NodesBatchDeletedHandler extends BaseEventHandler<NodesBatchDeletedPayload> {
    eventType = 'NodesBatchDeleted';

    handle(payload: NodesBatchDeletedPayload, callbacks?: EventCallbacks): void {
        this.log('Batch nodes deleted:', payload.nodeIds.length, 'from graph:', payload.graphId);
        const store = useGraphDataStore.getState();
        store.batchDeleteNodes(payload.nodeIds);

        if (callbacks?.onNodeDeleted) {
            for (const nodeId of payload.nodeIds) {
                callbacks.onNodeDeleted(payload.graphId, nodeId);
            }
        }
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

export class NodePinsUpdatedHandler extends BaseEventHandler<NodePinsUpdatedPayload> {
    eventType = 'NodePinsUpdated';

    handle(payload: NodePinsUpdatedPayload, _callbacks?: EventCallbacks): void {
        this.log('Node pins updated:', payload.nodeId, 'removed:', payload.removedPinIds.length, 'added:', payload.addedPins.length);

        useGraphDataStore.getState().batchUpdatePins({
            disconnectIds: payload.removedConnections.map(([from, to]) => `${from}->${to}`),
            removePinIds: payload.removedPinIds,
            addPins: payload.addedPins.map((pin) => ({
                nodeId: payload.nodeId,
                pin: pin as PinData,
            })),
        });
    }
}

export class PinTypesInferredHandler extends BaseEventHandler<PinTypesInferredPayload> {
    eventType = 'PinTypesInferred';

    handle(payload: PinTypesInferredPayload, _callbacks?: EventCallbacks): void {
        this.log('Pin types inferred:', payload.pinTypes.length, 'pins in graph:', payload.graphId);

        const store = useGraphDataStore.getState();
        for (const [pinId, resolvedType] of payload.pinTypes) {
            store.updatePin(pinId, { type: resolvedType });
        }
    }
}
