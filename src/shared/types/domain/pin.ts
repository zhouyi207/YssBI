/**
 * Domain Types - Pin
 *
 * Pin（针脚）是节点的输入输出接口
 * 用于节点之间的数据和控制流连接
 */

import type { DataType } from "./dataType";

/**
 * Pin 方向
 */
export type PinDirection = "input" | "output";

/**
 * 运行时 pin 种类（与 Rust `PinInstanceDTO.type` 对齐：仅 exec / object）。
 * 数据语义一律看 `dataType`。
 */
export type RuntimePinKind = "exec" | "object";

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
  dataType?: DataType;
}
