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
  eventPath: string;
}

/**
 * 函数作用域
 */
export interface FunctionScope {
  type: 'function';
  functionPath: string;
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
  resourcePath?: string;
  name: string;
  dataType: DataType;
  dataValue: DataValue;
  description: string;
  scope: VariableScope;
  tags: string[];
}

/**
 * 变量在某个图中是否可见（对齐后端 `GraphRuntime::variable_visible_in_graph`）。
 * - Global：任意图可见。
 * - Event / Function 局部：仅其所属图（且图类型匹配）可见。
 */
export function variableVisibleInGraph(
  scope: VariableScope,
  graphPath: string | undefined,
  graphKind: 'event' | 'function' | undefined,
): boolean {
  switch (scope.type) {
    case 'global':
      return true;
    case 'event':
      return graphKind === 'event' && !!graphPath && scope.eventPath === graphPath;
    case 'function':
      return graphKind === 'function' && !!graphPath && scope.functionPath === graphPath;
    default:
      return false;
  }
}
