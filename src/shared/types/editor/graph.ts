import { Connection } from "./connection";

export type GraphType = 'event' | 'function' | 'macro';

export type GraphPosition = {
  x: number;
  y: number;
  scale: number;
};

export interface Graph {
  id: string;
  name: string;
  type: GraphType;
  nodes: any[];
  pins: any[];
  connections: Connection;  // 注意：这是 ConnectionDTO 对象，不是数组
  canvas: GraphPosition;
}

// DTO 类型与 Graph 一致
export type GraphDTO = Graph;

// 前后端转换辅助函数
export const GraphConverter = {
  fromDTO(dto: GraphDTO): Graph {
    return dto;
  },

  toDTO(graph: Graph): GraphDTO {
    return graph;
  },
};