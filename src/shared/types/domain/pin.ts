/**
 * Domain Types - Pin
 * 
 * Pin（针脚）是节点的输入输出接口
 * 用于节点之间的数据和控制流连接
 */

/**
 * Pin 方向
 */
export type PinDirection = "input" | "output";

/**
 * Pin 类型标识符
 * 实际的类型定义（颜色、转换规则）从后端 schema 获取
 */
export type PinType =
    | "exec"      // 执行针脚（控制流）
    | "int"       // 整数
    | "float"     // 浮点数
    | "string"    // 字符串
    | "bool"      // 布尔
    | "object"    // 对象
    | "array"     // 数组
    | "struct"    // 结构体
    | "delegate"  // 委托/事件
    | string;     // 允许自定义类型

/**
 * Pin UI 配置
 * 用于渲染 Pin 的视觉样式
 */
export interface PinUI {
    x?: number;         // 在节点内部的 X 位置
    y?: number;         // 在节点内部的 Y 位置
    color?: string;     // 可选颜色（优先使用 schema 颜色）
}

/**
 * Pin 实例
 * 代表节点上的一个输入或输出接口
 */
export interface Pin {
    id: string;                 // 唯一标识
    nodeId: string;             // 所属节点 ID
    name: string;               // 显示名称
    type: PinType;              // 数据类型
    direction: PinDirection;    // 方向（输入/输出）
    links: string[];            // 连接的其他 Pin ID 列表
    defaultValue?: any;         // 默认值（数据针脚）
    userValue?: any;            // 用户设置的值（覆盖默认值）
    isArray?: boolean;          // 是否为数组/List 类型
    ui?: PinUI;                 // UI 配置
}
