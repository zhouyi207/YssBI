/**
 * Domain Types - Pin
 * 
 * Pin（针脚）是节点的输入输出接口
 * 用于节点之间的数据和控制流连接
 */

import type { DataType } from './dataType';

/**
 * Pin 方向
 */
export type PinDirection = "input" | "output";

/**
 * 运行时 pin 种类（与 Rust `PinInstanceDTO.type` 对齐：仅 exec / object）。
 * 数据语义一律看 `dataType`。
 */
export type RuntimePinKind = 'exec' | 'object';

/** @deprecated 使用 `RuntimePinKind`；宽 union 仅作历史文档参考。 */
export type PinType =
    | RuntimePinKind
    | "int"
    | "float"
    | "string"
    | "bool"
    | "array"
    | "struct"
    | "delegate"
    | string;

/**
 * Pin UI 配置
 * 用于渲染 Pin 的视觉样式
 */
export interface PinUI {
    x?: number;         // 在节点内部的 X 位置
    y?: number;         // 在节点内部的 Y 位置
    color?: string;     // 可选颜色（优先使用 schema 颜色）
}

/**
 * Pin 实例
 * 代表节点上的一个输入或输出接口
 */
export interface Pin {
    id: string;
    nodeId: string;
    name: string;
    type: RuntimePinKind;
    direction: PinDirection;
    defaultValue?: unknown;
    userValue?: unknown;
    dataType?: DataType;
    optional?: boolean;
    ui?: PinUI;
}
