// src/features/core/sync/utils/eventParser.ts

import { BaseEvent, NestedEvent } from '../types';

export interface ParsedEvent {
    type: string;
    payload: unknown;
}

/**
 * 解析后端事件结构
 * 处理嵌套事件：{ type: "Event", payload: { type: "EventCreated", payload: {...} } }
 */
export function parseEvent(event: BaseEvent | NestedEvent): ParsedEvent {
    const eventType = event.type;
    const eventPayload = event.payload;

    // 检查是否为嵌套事件
    if (eventPayload && typeof eventPayload === 'object' && 'type' in eventPayload && 'payload' in eventPayload) {
        const nested = eventPayload as { type: string; payload: unknown };
        return {
            type: nested.type,
            payload: nested.payload
        };
    }

    // 直接事件：Project
    return {
        type: eventType,
        payload: eventPayload
    };
}

/**
 * 验证事件类型
 */
export function isValidEventType(type: string): boolean {
    const validTypes = [
        // Project
        'ProjectLoaded', 'ProjectCleared', 'ProjectSaved',
        // Graph
        'EventCreated', 'EventUpdated', 'EventDeleted', 'EventCreatedFailed',
        'FunctionCreated', 'FunctionUpdated', 'FunctionDeleted', 'FunctionCreatedFailed',
        'MacroCreated', 'MacroUpdated', 'MacroDeleted', 'MacroCreatedFailed',
        // Variable
        'VariableCreated', 'VariableUpdated', 'VariableDeleted',
        // DataFrame
        'DataFrameCreated', 'DataFrameDeleted',
        // Node
        'NodeCreated', 'NodesBatchCreated', 'NodeUpdated', 'NodeDeleted', 'NodesBatchDeleted', 'NodePositionsUpdated', 'NodePinsUpdated',
        // Connection
        'ConnectionCreated', 'ConnectionDeleted', 'ConnectionsBatchDeleted',
    ];
    
    return validTypes.includes(type);
}
