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
    from_pin: string;  // 源 Pin ID（输出）
    to_pin: string;    // 目标 Pin ID（输入）
}

/**
 * 连接集合
 * 包含图中所有的连接关系
 */
export interface Connection {
    connections: ConnectionItem[];
}
