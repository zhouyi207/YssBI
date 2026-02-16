import { GraphData } from "./graph";
import { VariableData } from "./variable";

/**
 * Domain Types - Project
 * 
 * Project 代表整个项目的数据结构
 */

/**
 * 项目元数据
 */
export interface ProjectMetadata {
    exportTime: string;   // 导出时间
    appVersion: string;   // 应用版本
}

/**
 * 项目数据
 * 包含项目的所有内容
 */
export interface ProjectData {
    variables: Record<string, VariableData>;  // 变量集合（ID -> Variable）
    graphs: Record<string, GraphData>;        // 图集合（ID -> Graph）
    databases: Record<string, any>;       // 数据库集合（ID -> Database）
    metadata: ProjectMetadata;            // 元数据
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
