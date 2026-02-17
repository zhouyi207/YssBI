/**
 * Domain Types - Connection
 * 
 * Connection 代表两个 Pin 之间的连接关系
 */

/**
 * 单个连接
 * 表示从一个输出 Pin 到一个输入 Pin 的连接
 */
export interface ConnectionItem {
    fromPin: string;  // 源 Pin ID（输出）
    toPin: string;    // 目标 Pin ID（输入）
}

/**
 * 连接集合
 * 包含图中所有的连接关系
 */
export interface Connection {
    connections: ConnectionItem[];
}
