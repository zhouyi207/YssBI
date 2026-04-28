/**
 * Domain Types - Variable
 *
 * Variable 代表图中的变量定义
 */

import type { DataType } from './dataType';
import type { DataValue } from './dataValue';

/**
 * 全局作用域
 */
export interface GlobalScope {
  type: 'global';
}

/**
 * Event 作用域
 */
export interface EventScope {
  type: 'event';
  eventId: string;
}

/**
 * 函数作用域
 */
export interface FunctionScope {
  type: 'function';
  functionId: string;
}

/**
 * 变量作用域
 */
export type VariableScope = GlobalScope | EventScope | FunctionScope;

/**
 * 变量实例
 */
export interface Variable {
  id: string;
  name: string;
  dataType: DataType;
  dataValue: DataValue;
  description: string;
  scope: VariableScope;
  tags: string[];
}
