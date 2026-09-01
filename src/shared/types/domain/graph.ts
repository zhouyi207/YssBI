import { Connection } from "./connection";
import type { DataType } from "./dataType";
import { Node } from "./node";
import { Pin } from "./pin";

/**
 * Domain Types - Graph
 *
 * Graph 代表一个可执行的图（Event、Function）
 */

/**
 * 图类型
 */
export type GraphType = "event" | "function";

/**
 * 图实例
 * 代表一个完整的节点图（不含编辑器视口；视口为前端 EditorViewport）
 */
export interface Graph {
  /** 图资源相对路径（如 `events/Main.yssbi-event`） */
  path: string;
  name: string;
  type: GraphType;
  functionInputs?: FunctionSignaturePin[];
  functionOutputs?: FunctionSignaturePin[];
  nodes: Node[];
  pins: Pin[];
  connections: Connection;
}

export interface FunctionSignaturePin {
  id: string;
  name: string;
  /** 结构化类型；缺省表示 exec pin */
  dataType?: DataType;
}

export type FunctionPinSpec = FunctionSignaturePin;

export interface FunctionSignaturePatch {
  inputs?: FunctionPinSpec[];
  outputs?: FunctionPinSpec[];
}
