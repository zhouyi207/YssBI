// src/features/core/sync/hooks/useSyncManager.ts

import { useProjectSync } from './useProjectSync';
import { EventCallbacks } from '../types';

/**
 * 统一的同步管理器
 * 可以同时管理多个监听器
 */
export function useSyncManager(options: {
    project?: boolean | EventCallbacks;
    execution?: boolean;
} = {}) {
    const { project = true } = options;

    // 项目同步
    useProjectSync(
        typeof project === 'object' ? project : project ? {} : undefined
    );

    // 执行同步（如果需要）
    // useExecutionSync(execution);

    // 可以添加更多监听器...
}
