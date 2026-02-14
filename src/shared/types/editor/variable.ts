import { DataType } from "./datatype";

export interface GlobalScope {
  type: "global";
}

export interface FunctionScope {
  type: "function";
  function_id: string;
}

export interface MacroScope {
  type: "macro";
  macro_id: string;
}

/** 变量作用域 */
export type VariableScope = GlobalScope | FunctionScope | MacroScope;

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

// DTO 类型与 Variable 一致
export type VariableDTO = Variable;

// 前后端转换辅助函数
export const VariableConverter = {
  fromDTO(dto: VariableDTO): Variable {
    return dto;
  },

  toDTO(variable: Variable): VariableDTO {
    return variable;
  },
};