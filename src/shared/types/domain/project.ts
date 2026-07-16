import type { Graph } from "./graph";
import type { Variable } from "./variable";
import type { DatabaseDecl } from "./database";

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
    graphs: Record<string, Graph>;        // 图集合（path -> Graph，键与 Graph.path 一致）
    databases: Record<string, DatabaseDecl>;  // 数据库集合（ID -> DatabaseDecl）
    metadata: ProjectMetadata;            // 元数据
}