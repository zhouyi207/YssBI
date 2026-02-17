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
  isArray?: boolean;
}

/** 序列化后的节点 */
export interface SerializedNode {
  id: string;
  type: string;
  title: string;
  position: { x: number; y: number };
  isInternal?: boolean;
  variableId?: string;
  variableType?: string;
  variableName?: string;
  subGraphId?: string;
  inputs: SerializedPin[];
  outputs: SerializedPin[];
}

/** 序列化后的图数据 */
export interface SerializedGraphData {
  id: string;
  name: string;
  type: 'event' | 'function' | 'macro';
  canvas: GraphPosition;
  variables: Record<string, Variable>;
  inputs: Pin[];
  outputs: Pin[];
  connections: { connections: Array<{ fromPin: string; toPin: string }> };
  nodes: SerializedNode[];
}

/** 反序列化输入（可能来自 Store 或后端） */
export interface DeserializeGraphInput {
  nodes?: Array<{
    id: string;
    type?: string;
    node_type?: string;
    category?: string[];
    title?: string;
    position?: { x: number; y: number };
    ui_style?: string;
    description?: string;
    isInternal?: boolean;
    subGraphId?: string;
    variableId?: string;
    variableType?: string;
    variableName?: string;
    inputs?: (string | SerializedPin)[];
    outputs?: (string | SerializedPin)[];
  }>;
  pins?: SerializedPin[];
  connections?: { connections?: Array<{ fromPin?: string; toPin?: string; from?: string; to?: string }> } | Array<{ fromPin?: string; toPin?: string; from?: string; to?: string }>;
  canvas?: GraphPosition;
}

/** 反序列化后的 Pin（含 nodeId, direction, links） */
export interface DeserializedPin extends SerializedPin {
  nodeId: string;
  direction: 'input' | 'output';
  links: string[];
}

/** 反序列化后的节点（运行时格式，含 links） */
export interface DeserializedNode {
  id: string;
  type: string;
  node_type: string;
  category: string[];
  title: string;
  position: { x: number; y: number };
  inputs: DeserializedPin[];
  outputs: DeserializedPin[];
  ui_style: string;
  description?: string;
  isInternal?: boolean;
  subGraphId?: string;
  variableId?: string;
  variableType?: string;
  variableName?: string;
}
