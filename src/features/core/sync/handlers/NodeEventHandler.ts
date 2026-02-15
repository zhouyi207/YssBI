// src/features/core/sync/handlers/NodeEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import { NodeCreatedPayload, NodeDeletedPayload, EventCallbacks } from '../types';
import { useNodeStore } from '@/features/core/_node/useNodeStore';

export class NodeCreatedHandler extends BaseEventHandler<NodeCreatedPayload> {
    eventType = 'NodeCreated';
    
    handle(payload: NodeCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Node created:', payload.node_id, 'in graph:', payload.graph_id);
        
        // 通过回调通知业务逻辑层处理
        // 因为需要将后端的 NodeInstanceDTO 转换为前端的 Node 对象
        callbacks?.onNodeCreated?.(payload.graph_id, payload.node_id, payload.data);
    }
}

export class NodeDeletedHandler extends BaseEventHandler<NodeDeletedPayload> {
    eventType = 'NodeDeleted';
    
    handle(payload: NodeDeletedPayload, callbacks?: EventCallbacks): void {
        this.log('Node deleted:', payload.node_id, 'from graph:', payload.graph_id);
        
        // 通过回调通知业务逻辑层处理
        callbacks?.onNodeDeleted?.(payload.graph_id, payload.node_id);
    }
}
