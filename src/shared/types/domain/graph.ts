import { Connection } from "./connection";
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
export type GraphType = 'event' | 'function';

/**
 * 图的视口位置
 * 用于记录画布的平移和缩放状态
 */
export interface GraphPosition {
    x: number;      // X 轴偏移
    y: number;      // Y 轴偏移
    scale: number;  // 缩放比例
}

/**
 * 图实例
 * 代表一个完整的节点图
 */
export interface Graph {
    id: string;                 // 图 ID
    name: string;               // 图名称
    type: GraphType;            // 图类型
    nodes: Node[];              // 节点列表
    pins: Pin[];                // Pin 列表（所有节点的 Pin）
    connections: Connection;    // 连接关系
    canvas: GraphPosition;      // 画布状态
}
