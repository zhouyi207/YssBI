/**
 * 序列化相关类型
 * 用于 serializeGraph / deserializeGraph 的输入输出
 */

import type { GraphPosition } from '../domain/graph';
import type { Pin } from '../domain/pin';
import type { Variable } from '../domain/variable';

/** 序列化后的 Pin（不含 links） */
export interface SerializedPin {
  id: string;
  name: string;
  type: string;
  defaultValue?: unknown;
  userValue?: unknown;
  containerType?: string;
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
  subGraphId?: string;
  dataframeId?: string;
  inputs: SerializedPin[];
  outputs: SerializedPin[];
}

/** 序列化后的图数据 */
export interface SerializedGraphData {
  id: string;
  name: string;
  type: 'event' | 'function';
  canvas: GraphPosition;
  variables: Record<string, Variable>;
  inputs: Pin[];
  outputs: Pin[];
  connections: { connections: Array<{ fromPin: string; toPin: string }> };
  nodes: SerializedNode[];
}

/** 反序列化输入（来自后端 DTO，camelCase） */
export interface DeserializeGraphInput {
  nodes?: Array<{
    id: string;
    nodeType?: string;
    category?: string[];
    title?: string;
    position?: { x: number; y: number };
    uiStyle?: string;
    description?: string;
    isInternal?: boolean;
    subGraphId?: string;
    variableId?: string;
    variableType?: string;
    variableName?: string;
    dataframeId?: string;
    inputs?: (string | SerializedPin)[];
    outputs?: (string | SerializedPin)[];
  }>;
  pins?: SerializedPin[];
  connections?: { connections?: Array<{ fromPin?: string; toPin?: string; from?: string; to?: string }> } | Array<{ fromPin?: string; toPin?: string; from?: string; to?: string }>;
  canvas?: GraphPosition;
}

/** 反序列化后的 Pin 运行时视图（links 从 connections 派生） */
export interface DeserializedPin extends SerializedPin {
  nodeId: string;
  direction: 'input' | 'output';
  links: string[];
}

/** 反序列化后的节点（运行时格式，含 links，camelCase） */
export interface DeserializedNode {
  id: string;
  nodeType: string;
  category: string[];
  title: string;
  position: { x: number; y: number };
  inputs: DeserializedPin[];
  outputs: DeserializedPin[];
  uiStyle: string;
  description?: string;
  isInternal?: boolean;
  subGraphId?: string;
  variableId?: string;
  variableType?: string;
  variableName?: string;
  dataframeId?: string;
}
