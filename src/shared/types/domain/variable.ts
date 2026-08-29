/**
 * Domain Types - Variable
 *
 * Variable 代表图中的变量定义
 */

import type { DataType } from './dataType';
import { dataTypeFromKey } from './dataType';
import type { DataValue } from './dataValue';
import { inferDataValueFromJson } from './dataValue';

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

type VariablePayload = {
  id: string;
  resourcePath?: string;
  name?: unknown;
  dataType?: unknown;
  dataValue?: unknown;
  description?: unknown;
  scope?: unknown;
  tags?: unknown;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function dataTypeFromBackend(value: unknown): DataType {
  if (typeof value === 'string') return dataTypeFromKey(value);
  if (!isRecord(value) || typeof value.kind !== 'string') return { kind: 'Any' };
  if (value.kind === 'Array' && value.inner !== undefined) {
    return { kind: 'Array', inner: dataTypeFromBackend(value.inner) };
  }
  if (value.kind === 'DataSeries' && value.inner !== undefined) {
    return { kind: 'DataSeries', inner: dataTypeFromBackend(value.inner) };
  }
  if (value.kind === 'OneOf' && Array.isArray(value.inner)) {
    return { kind: 'OneOf', inner: value.inner.map(dataTypeFromBackend) };
  }
  if (value.kind === 'Struct' && typeof value.inner === 'string') {
    return { kind: 'Struct', inner: value.inner };
  }
  return dataTypeFromKey(value.kind);
}

function dataValueFromBackend(value: unknown): DataValue {
  if (value == null || value === 'Null') return { kind: 'Null' };
  if (!isRecord(value)) return { kind: 'Null' };
  if ('kind' in value && 'value' in value) return value as DataValue;
  if ('Boolean' in value && typeof value.Boolean === 'boolean') {
    return { kind: 'Boolean', value: value.Boolean };
  }
  if ('Int64' in value && typeof value.Int64 === 'number') {
    return { kind: 'Int64', value: value.Int64 };
  }
  if ('Float64' in value && typeof value.Float64 === 'number') {
    return { kind: 'Float64', value: value.Float64 };
  }
  if ('String' in value && typeof value.String === 'string') {
    return { kind: 'String', value: value.String };
  }
  if ('Array' in value && Array.isArray(value.Array)) {
    return { kind: 'Array', value: value.Array.map(dataValueFromBackend) };
  }
  if ('Object' in value && isRecord(value.Object)) {
    return { kind: 'Object', value: value.Object };
  }
  if ('DataFrame' in value && typeof value.DataFrame === 'string') {
    return { kind: 'DataFrame', value: value.DataFrame };
  }
  if ('DataSeries' in value) {
    if (typeof value.DataSeries === 'string') {
      return { kind: 'DataSeries', value: value.DataSeries };
    }
    if (isRecord(value.DataSeries) && typeof value.DataSeries.id === 'string') {
      return {
        kind: 'DataSeries',
        value: {
          id: value.DataSeries.id,
          ...(value.DataSeries.elementType === undefined
            ? {}
            : { elementType: dataTypeFromBackend(value.DataSeries.elementType) }),
        },
      };
    }
  }
  if ('Struct' in value && isRecord(value.Struct)
    && typeof value.Struct.typeKey === 'string'
    && typeof value.Struct.handleId === 'string') {
    return { kind: 'Struct', value: value.Struct as { typeKey: string; handleId: string } };
  }
  return inferDataValueFromJson(value);
}

export function normalizeVariableFromBackend(raw: VariablePayload): Variable {
  return {
    id: raw.id,
    resourcePath: raw.resourcePath,
    name: typeof raw.name === 'string' ? raw.name : raw.id,
    dataType: dataTypeFromBackend(raw.dataType),
    dataValue: dataValueFromBackend(raw.dataValue),
    description: typeof raw.description === 'string' ? raw.description : '',
    scope: isRecord(raw.scope) ? raw.scope as unknown as VariableScope : { type: 'global' },
    tags: Array.isArray(raw.tags)
      ? raw.tags.filter((tag): tag is string => typeof tag === 'string')
      : [],
  };
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
