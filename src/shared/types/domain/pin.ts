/**
 * Domain Types - Pin
 *
 * Pin（针脚）是节点的输入输出接口
 * 用于节点之间的数据依赖连接
 */

import type { PortTypeStateDto } from "./editorProjection";

/**
 * Pin 方向
 */
export type PinDirection = "input" | "output";

/**
 * Pin 实例
 * 代表节点上的一个输入或输出接口
 */
export interface Pin {
  id: string;
  nodeId: string;
  name: string;
  direction: PinDirection;
  typeState: PortTypeStateDto;
}
