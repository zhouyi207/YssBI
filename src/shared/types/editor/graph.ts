export type PinDirection = "input" | "output";

/**
 * Pin 类型标识符
 * 实际的类型定义（颜色、转换规则）从后端 schema 获取
 */
export type PinType =
    | "exec"      // 执行针脚
    | "int"       // 整数
    | "float"     // 浮点数
    | "string"    // 字符串
    | "bool"      // 布尔
    | "object"    // 对象
    | "array"     // 数组
    | "struct"    // 结构体
    | "delegate"  // 委托/事件
    | string;     // 允许自定义类型


export interface Pin {
    id: string;           // 唯一标识
    nodeId: string;       // 所属节点 ID
    name: string;         // 显示名称
    type: PinType;        // 类型
    direction: PinDirection;
    links: string[];      // 连接的 pin id
    defaultValue?: any;   // 默认值（如果是数据针脚）
    userValue?: any;      // 🆕 用户设置的值（覆盖默认值）
    isArray?: boolean;    // 是否为数组/List
    ui?: {
        x?: number;         // 在节点内部的位置
        y?: number;
        color?: string;     // 可选，按类型渲染（优先使用 schema 颜色）
    };
}

export interface Node {
    id: string;
    node_type: string;
    category: string[];
    title: string;
    inputs: Pin[];
    outputs: Pin[];
    ui_style: string;
    description?: string;
}

export interface Connection {
  id: string;           // Unique identifier for the connection
  sourcePin: string;    // ID of the source (output) pin
  targetPin: string;    // ID of the target (input) pin
}


export type GraphPosition = {
  x: number;
  y: number;
  scale: number;
};

export interface Graph {
  id: string;
  name: string;
  type: "event" | "function" | "macro";
  nodes: any[];
  pins: any[];
  connections: Connection[];
  canvas: GraphPosition;
}