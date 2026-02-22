/**
 * UI Types - Common
 * 
 * 通用 UI 类型
 */

/**
 * 位置坐标
 */
export interface Position {
    x: number;
    y: number;
}

/**
 * 尺寸
 */
export interface Size {
    width: number;
    height: number;
}

/**
 * 矩形区域
 */
export interface Rect extends Position, Size {}

/**
 * 加载状态
 */
export enum LoadStatus {
    Idle = "idle",
    Loading = "loading",
    Ready = "ready",
    Error = "error",
}

/**
 * 日志级别
 */
export enum LogLevel {
    Trace = "trace",
    Debug = "debug",
    Info = "info",
    Warn = "warn",
    Error = "error",
}

/**
 * 日志类型
 */
export enum LogType {
    Application = "application",
    Execution = "execution",
    System = "system",
    Graph = "graph",
    Data = "data",
}

/**
 * 日志消息
 */
export interface LogMessage {
    timestamp: string;
    level: LogLevel;
    log_type: LogType;
    message: string;
    source?: string;
}

/**
 * 日志过滤器
 */
export interface LogFilter {
    levels: Set<LogLevel>;
    types: Set<LogType>;
    source?: string;
    searchText?: string;
}
