/// helpers —— 非 React 的纯函数

import { useSchemaStore } from "./shema.store";
import { LoadStatus } from "@/shared/types/loadStatus";
import { PinTypeDefinition, CategoryDefinition, VariableTypeDefinition, UIStyleDefinition } from "./shema.types";

/**
 * 获取 Pin 类型定义（只读）
 *
 * - Schema 未 Ready 时返回 undefined
 */
export function getPinType(name: string): PinTypeDefinition | undefined {
  const { status, getPinType } = useSchemaStore.getState();

  if (status !== LoadStatus.Ready) {
    return undefined;
  }

  return getPinType(name);
}

/**
 * 获取分类定义（只读）
 *
 * - Schema 未 Ready 时返回 undefined
 */
export function getCategory(name: string): CategoryDefinition | undefined {
  const { status, getCategory } = useSchemaStore.getState();

  if (status !== LoadStatus.Ready) {
    return undefined;
  }

  return getCategory(name);
}

/**
 * 获取变量类型定义（只读）
 *
 * - Schema 未 Ready 时返回 undefined
 */
export function getVariableType(name: string): VariableTypeDefinition | undefined {
  const { status, getVariableType } = useSchemaStore.getState();

  if (status !== LoadStatus.Ready) {
    return undefined;
  }

  return getVariableType(name);
}

/**
 * 获取 UI 样式定义（只读）
 *
 * - Schema 未 Ready 时返回 undefined
 */
export function getUIStyle(name: string): UIStyleDefinition | undefined {
  const { status, getUIStyle } = useSchemaStore.getState();

  if (status !== LoadStatus.Ready) {
    return undefined;
  }

  return getUIStyle(name);
}

/**
 * 检查两个 Pin 类型是否可以连接
 *
 * 规则：
 * 1. 相同类型可以连接
 * 2. object 可以接受任何非 exec 类型
 * 3. 检查隐式转换规则
 *
 * - Schema 未 Ready 时返回 false
 */
export function canConnect(fromType: string, toType: string): boolean {
  const { status, getPinType } = useSchemaStore.getState();

  if (status !== LoadStatus.Ready) {
    return false;
  }

  // 相同类型
  if (fromType === toType) return true;

  // object 可以接受任何非 exec 类型
  if (toType === "object" && fromType !== "exec") return true;

  // 检查隐式转换
  const fromDef = getPinType(fromType);
  if (fromDef && fromDef.implicit_convert_to.includes(toType)) {
    return true;
  }

  return false;
}

/**
 * 获取节点的中心符号
 *
 * - Schema 未 Ready 时返回 undefined
 */
export function getCenterSymbol(styleName: string, nodeType: string): string | undefined {
  const { status, getUIStyle } = useSchemaStore.getState();

  if (status !== LoadStatus.Ready) {
    return undefined;
  }

  const style = getUIStyle(styleName);
  return style?.center_symbols[nodeType];
}

/**
 * 获取可见分类列表（按 sort_order 排序）
 *
 * - Schema 未 Ready 时返回空数组
 */
export function getVisibleCategories(): CategoryDefinition[] {
  const { status, getVisibleCategories } = useSchemaStore.getState();

  if (status !== LoadStatus.Ready) {
    return [];
  }

  return getVisibleCategories();
}

/**
 * 获取所有 Pin 类型
 *
 * - Schema 未 Ready 时返回空数组
 */
export function getAllPinTypes(): PinTypeDefinition[] {
  const { status, getAllPinTypes } = useSchemaStore.getState();

  if (status !== LoadStatus.Ready) {
    return [];
  }

  return getAllPinTypes();
}

/**
 * 获取所有变量类型
 *
 * - Schema 未 Ready 时返回空数组
 */
export function getAllVariableTypes(): VariableTypeDefinition[] {
  const { status, getAllVariableTypes } = useSchemaStore.getState();

  if (status !== LoadStatus.Ready) {
    return [];
  }

  return getAllVariableTypes();
}
