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
