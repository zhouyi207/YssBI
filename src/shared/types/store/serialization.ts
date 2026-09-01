/**
 * 序列化相关类型（历史遗留；连接事实在 connections / pinConnections）
 */

import type { Variable } from "../domain/variable";

/** 序列化后的 Pin（不含连接状态） */
export interface SerializedPin {
  id: string;
  name: string;
  type: string;
  defaultValue?: unknown;
  userValue?: unknown;
}

/** 序列化后的节点（camelCase 与后端 DTO 一致） */
export interface SerializedNode {
  id: string;
  nodeType: string;
  title: string;
  position: { x: number; y: number };
  isInternal?: boolean;
  variableId?: string;
  variableType?: string;
  variableName?: string;
  subGraphPath?: string;
  dataframeId?: string;
  inputs: SerializedPin[];
  outputs: SerializedPin[];
}

/** 序列化后的图数据 */
export interface SerializedGraphData {
  id: string;
  name: string;
  type: "event" | "function";
  variables: Record<string, Variable>;
  connections: { connections: Array<{ fromPin: string; toPin: string }> };
  nodes: SerializedNode[];
}
