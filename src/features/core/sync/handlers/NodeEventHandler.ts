// src/features/core/sync/handlers/NodeEventHandler.ts
// 按 README 范式：Handler 直接更新 Store，callbacks 仅用于可选 UI 扩展

import { BaseEventHandler } from './BaseEventHandler';
import { NodeCreatedPayload, NodesBatchCreatedPayload, NodeDeletedPayload, NodesBatchDeletedPayload, NodePositionsUpdatedPayload, NodePinsUpdatedPayload, PinTypesInferredPayload, RuntimeSourcesInvalidatedPayload, EventCallbacks } from '../types';
import { useGraphDataStore } from '@/features/core/dataStore';
import { resolveNodeViewMeta } from '@/features/domain/nodeViewMeta';
import { useExecutionStore } from '@/features/core/execution';
import { markGraphTabDirty } from '@/features/core/layout/tabDirty';
import { shouldSuppressGraphRefreshEcho } from '@/features/application/graphDocument/graphRefreshEchoGuard';
import { isPending } from '../utils/echoSuppressor';
import { NODE_POSITION_ECHO_DOMAIN } from '@/features/core/history/commands/moveNodes';
import type { NodeData, PinData } from '@/shared/types';
import type { NodeInstanceDTO } from '@/shared/types/dto';
import { flattenInstanceParams } from '@/shared/types/dto/nodeInstanceParams';
import { runtimePinRefsToIds } from '@/shared/types/dto/graphModel';
import { dataTypeFromBackend } from '@/shared/types/dto/dataType';
import { normalizePinDto, pinInferredPatch } from '@/shared/types/dto/pinHydrate';

function dtoToNodeData(graphPath: string, nodeId: string, d: NodeInstanceDTO): NodeData {
    const meta = resolveNodeViewMeta({
        nodeType: d.nodeType,
        title: d.title,
        category: d.category,
        description: d.description,
    });
    const params = flattenInstanceParams(d);
    return {
        id: nodeId,
        graphPath,
        nodeType: meta.nodeType,
        category: meta.category,
        title: meta.title,
        inputs: runtimePinRefsToIds(d.inputs),
        outputs: runtimePinRefsToIds(d.outputs),
        description: meta.description,
        position: d.position ?? { x: 0, y: 0 },
        paramsKind: params.paramsKind,
        variableId: params.variableId,
        variableName: params.variableName,
        variableType: params.variableType,
        subGraphPath: params.subGraphPath,
        dataframeId: params.dataframeId,
    };
}

export class NodeCreatedHandler extends BaseEventHandler<NodeCreatedPayload> {
    eventType = 'NodeCreated';

    handle(payload: NodeCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Node created:', payload.nodeId, 'in graph:', payload.graphPath);

        // reconcileNode 同时处理两种情形：
        // - 本地已乐观插入（创建命令，id 一致）：用后端权威字段覆盖，无重复节点；
        // - 节点尚不存在（redo / 其它来源）：按普通添加路径插入。
        const store = useGraphDataStore.getState();
        store.reconcileNode(
            payload.graphPath,
            dtoToNodeData(payload.graphPath, payload.nodeId, payload.data),
            payload.pins.map((pin) => normalizePinDto(pin as PinData)),
        );
        markGraphTabDirty(payload.graphPath);
        callbacks?.onNodeCreated?.(payload.graphPath, payload.nodeId, payload.data);
    }
}

export class NodesBatchCreatedHandler extends BaseEventHandler<NodesBatchCreatedPayload> {
    eventType = 'NodesBatchCreated';

    handle(payload: NodesBatchCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Batch nodes created:', payload.nodes.length, 'in graph:', payload.graphPath);

        const store = useGraphDataStore.getState();

        const items = payload.nodes.map(([nodeId, data, pins]) => ({
            node: dtoToNodeData(payload.graphPath, nodeId, data),
            pins: pins.map((pin) => normalizePinDto(pin as PinData)),
        }));

        store.batchAddNodesAndPins(payload.graphPath, items);
        markGraphTabDirty(payload.graphPath);

        if (callbacks?.onNodeCreated) {
            for (const [nodeId, data] of payload.nodes) {
                callbacks.onNodeCreated(payload.graphPath, nodeId, data);
            }
        }
    }
}

export class NodeDeletedHandler extends BaseEventHandler<NodeDeletedPayload> {
    eventType = 'NodeDeleted';

    handle(payload: NodeDeletedPayload, callbacks?: EventCallbacks): void {
        this.log('Node deleted:', payload.nodeId, 'from graph:', payload.graphPath);
        const store = useGraphDataStore.getState();
        if (store.getGraphNode(payload.graphPath, payload.nodeId)) {
            store.deleteNode(payload.nodeId, payload.graphPath);
        }
        markGraphTabDirty(payload.graphPath);
        callbacks?.onNodeDeleted?.(payload.graphPath, payload.nodeId);
    }
}

export class NodesBatchDeletedHandler extends BaseEventHandler<NodesBatchDeletedPayload> {
    eventType = 'NodesBatchDeleted';

    handle(payload: NodesBatchDeletedPayload, callbacks?: EventCallbacks): void {
        this.log('Batch nodes deleted:', payload.nodeIds.length, 'from graph:', payload.graphPath);
        const store = useGraphDataStore.getState();
        store.batchDeleteNodes(payload.nodeIds, payload.graphPath);
        markGraphTabDirty(payload.graphPath);

        if (callbacks?.onNodeDeleted) {
            for (const nodeId of payload.nodeIds) {
                callbacks.onNodeDeleted(payload.graphPath, nodeId);
            }
        }
    }
}

export class NodePositionsUpdatedHandler extends BaseEventHandler<NodePositionsUpdatedPayload> {
    eventType = 'NodePositionsUpdated';

    handle(payload: NodePositionsUpdatedPayload, _callbacks?: EventCallbacks): void {
        // Skip echoes for nodes whose move command issued by this client is
        // still in-flight: we already applied the positions optimistically and
        // re-applying a stale snapshot from the backend can briefly revert the
        // UI when several drags overlap. Updates that arrive from another
        // origin (other window, undo/redo from elsewhere) are still applied.
        const updates = payload.updates
            .filter(([nodeId]) => !isPending(NODE_POSITION_ECHO_DOMAIN, nodeId))
            .map(([nodeId, x, y]) => ({ nodeId, x, y }));

        if (updates.length === 0) {
            this.log('Node positions updated (all suppressed as self-echo):', payload.graphPath);
            // Still mark dirty so save UI reflects the change.
            markGraphTabDirty(payload.graphPath);
            return;
        }

        this.log('Node positions updated:', payload.graphPath, updates.length, 'nodes');
        useGraphDataStore.getState().batchUpdateNodePositions(updates, payload.graphPath);
        markGraphTabDirty(payload.graphPath);
    }
}

export class NodePinsUpdatedHandler extends BaseEventHandler<NodePinsUpdatedPayload> {
    eventType = 'NodePinsUpdated';

    handle(payload: NodePinsUpdatedPayload, _callbacks?: EventCallbacks): void {
        if (shouldSuppressGraphRefreshEcho(payload.graphPath)) {
            this.log('Node pins updated (suppressed — invoke refresh authoritative):', payload.graphPath);
            return;
        }

        this.log(
            'Node pins updated:',
            payload.nodeId,
            'removed:',
            payload.removedPinIds.length,
            'added:',
            payload.addedPins.length,
            'updated:',
            payload.updatedPins?.length ?? 0,
        );

        const store = useGraphDataStore.getState();
        store.batchUpdatePins({
            disconnectIds: payload.removedConnections.map(([from, to]) => `${from}->${to}`),
            removePinIds: payload.removedPinIds,
            updatePins: (payload.updatedPins ?? []).map((pin) => {
                const normalized = normalizePinDto(pin);
                return {
                    pinId: pin.id,
                    patch: {
                        name: normalized.name,
                        type: normalized.type,
                        direction: normalized.direction,
                        dataType: normalized.dataType,
                    },
                };
            }),
            addPins: payload.addedPins.map((pin) => ({
                nodeId: payload.nodeId,
                pin: normalizePinDto(pin),
            })),
            graphPath: payload.graphPath,
        });

        if (payload.pinOrder) {
            store.reorderNodePins(payload.nodeId, payload.pinOrder, payload.graphPath);
        }
        markGraphTabDirty(payload.graphPath);
    }
}

export class PinTypesInferredHandler extends BaseEventHandler<PinTypesInferredPayload> {
    eventType = 'PinTypesInferred';

    handle(payload: PinTypesInferredPayload, _callbacks?: EventCallbacks): void {
        this.log('Pin types inferred:', payload.pinTypes.length, 'pins in graph:', payload.graphPath);

        useGraphDataStore.getState().batchUpdatePinFields(
            payload.pinTypes.map(({ pinId, dataType }) => ({
                pinId,
                patch: pinInferredPatch(dataTypeFromBackend(dataType)),
            })),
            payload.graphPath,
        );
        markGraphTabDirty(payload.graphPath);
    }
}

export class RuntimeSourcesInvalidatedHandler extends BaseEventHandler<RuntimeSourcesInvalidatedPayload> {
    eventType = 'RuntimeSourcesInvalidated';

    handle(payload: RuntimeSourcesInvalidatedPayload, _callbacks?: EventCallbacks): void {
        this.log(
            'Runtime sources invalidated:',
            payload.pinIds.length,
            'pins in graph:',
            payload.graphPath,
        );
        useExecutionStore.getState().clearPinResults(payload.graphPath, payload.pinIds);
    }
}
