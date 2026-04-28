// src/features/core/sync/utils/eventParser.ts

import { BaseEvent, NestedEvent } from '../types';

export interface ParsedEvent {
    type: string;
    payload: unknown;
}

/** 顶层事件分类（需递归解析到具体类型） */
const TOP_LEVEL_TYPES = new Set([
    'Project', 'Event', 'Function', 'Variable', 'Node', 'Connection', 'DataFrame',
]);

/**
 * 解析后端事件结构
 * 递归处理嵌套：Event { type: "Connection", payload: { type: "ConnectionsBatchDeleted", payload: {...} } }
 */
export function parseEvent(event: BaseEvent | NestedEvent): ParsedEvent {
    let current: { type: string; payload?: unknown } = event as { type: string; payload?: unknown };

    // 递归解析直到得到具体事件类型（非顶层分类）
    while (current?.payload && typeof current.payload === 'object' && 'type' in current.payload && 'payload' in current.payload) {
        const nested = current.payload as { type: string; payload: unknown };
        if (TOP_LEVEL_TYPES.has(nested.type)) {
            current = nested;
        } else {
            return { type: nested.type, payload: nested.payload };
        }
    }

    return {
        type: current.type,
        payload: current.payload
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
        // Variable
        'VariableCreated', 'VariableUpdated', 'VariableDeleted',
        // DataFrame
        'DataFrameCreated', 'DataFrameDeleted',
        // Node
        'NodeCreated', 'NodesBatchCreated', 'NodeUpdated', 'NodeDeleted', 'NodesBatchDeleted', 'NodePositionsUpdated', 'NodePinsUpdated', 'PinTypesInferred',
        // Connection
        'ConnectionCreated', 'ConnectionDeleted', 'ConnectionsBatchDeleted',
    ];
    
    return validTypes.includes(type);
}
