import { Pin, PinDirection } from "./pin";

export type { PinDirection };

/**
 * Domain Types - Node
 *
 * 这些类型代表核心业务领域模型，与后端数据结构一致
 */

export interface Node {
  id: string;
  nodeType: string;
  category: string[];
  title: string;
  inputs: Pin[];
  outputs: Pin[];
}

export interface NodePosition {
  x: number;
  y: number;
}
