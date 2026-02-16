import { Graph } from "./graph";
import { Variable } from "./variable";

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
    variables: Record<string, Variable>;  // 变量集合（ID -> Variable）
    graphs: Record<string, Graph>;        // 图集合（ID -> Graph）
    databases: Record<string, any>;       // 数据库集合（ID -> Database）
    metadata: ProjectMetadata;            // 元数据
}

// Backward-compat: these are frontend state types, re-exported from `shared/types/state`.
export type { ProjectState, ProjectEventPayload, UseProjectSyncOptions } from "../state/project";
