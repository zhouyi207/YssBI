/// types —— 只放"结构定义"

import { LoadStatus } from "@/shared/types/ui";

/**
 * Project 初始化状态
 */
export interface ProjectState {
    status: LoadStatus;
    error: string | null;
}

/**
 * 项目事件类型（与后端 ProjectEvent 对应）
 */
export interface ProjectEventPayload {
    type: string;
    payload: any;
}

/**
 * useProjectSync 配置
 */
export interface UseProjectSyncOptions {
    /** 是否启用同步 */
    enabled?: boolean;
    /** 项目加载回调 */
    onProjectLoaded?: (data: any, path: string | null) => void;
    /** 项目清除回调 */
    onProjectCleared?: () => void;
    /** 项目保存回调 */
    onProjectSaved?: (path: string) => void;
    /** Event 创建回调 */
    onEventCreated?: (id: string, data: any) => void;
    /** Function 创建回调 */
    onFunctionCreated?: (id: string, data: any) => void;
    /** Macro 创建回调 */
    onMacroCreated?: (id: string, data: any) => void;
}
