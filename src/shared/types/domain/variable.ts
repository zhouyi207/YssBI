/**
 * Domain Types - Variable
 * 
 * Variable 代表图中的变量定义
 */

/**
 * 数据类型
 */
export type DataType =
    | "Boolean"
    | "Int32"
    | "Int64"
    | "Float32"
    | "Float64"
    | "String"
    | "Array"
    | "Object"
    | "Any"
    | "Null"
    | "DataFrame";

/**
 * 全局作用域
 */
export interface GlobalScope {
    type: "global";
}

/**
 * 函数作用域
 */
export interface FunctionScope {
    type: "function";
    function_id: string;
}

/**
 * 宏作用域
 */
export interface MacroScope {
    type: "macro";
    macro_id: string;
}

/**
 * 变量作用域
 */
export type VariableScope = GlobalScope | FunctionScope | MacroScope;

/**
 * 变量实例
 * 代表图中的一个变量定义
 */
export interface Variable {
    /** 变量 ID */
    id: string;

    /** 变量名称 */
    name: string;

    /** 数据类型 */
    data_type: DataType;

    /** 描述 */
    description: string;

    /** 变量作用域 */
    scope: VariableScope;

    /** 变量值（简单类型） */
    static_value?: unknown;

    /** 是否为数组 */
    is_array?: boolean;

    /** 是否为常量 */
    is_constant?: boolean;

    /** 默认值 */
    default_value?: unknown;

    /** 是否暴露给外部 */
    is_exposed?: boolean;

    /** 标签 */
    tags?: string[];
}
