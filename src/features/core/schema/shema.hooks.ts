/// hooks —— 生命周期 + 组合逻辑（重点）

import { useEffect } from "react";
import { useSchemaStore } from "./shema.store";
import { SchemaState } from "@/shared/types/state";
import { LoadStatus } from "@/shared/types/ui";

/**
 * Schema 初始化 Hook
 *
 * 语义：
 * - 首次使用时自动触发初始化
 * - 返回标准的 SchemaState（status + error）
 */
export function useSchema(): SchemaState {
  const status = useSchemaStore((s) => s.status);
  const error = useSchemaStore((s) => s.error);
  const syncFromBackend = useSchemaStore((s) => s.syncFromBackend);

  useEffect(() => {
    if (status === LoadStatus.Idle) {
      syncFromBackend().catch((err) => {
        console.error("[useSchema] Initialization failed:", err);
      });
    }
  }, [status, syncFromBackend]);

  return {
    status,
    error,
  };
}

/**
 * 获取所有 Pin 类型（安全）
 *
 * - 未 Ready 时返回空数组
 * - 调用方无需关心初始化时序
 */
export function usePinTypes() {
  const { status, error } = useSchema();
  const pinTypes = useSchemaStore((s) => s.getAllPinTypes());

  return {
    pinTypes: status === LoadStatus.Ready ? pinTypes : [],
    status,
    error,
  };
}

/**
 * 获取所有分类（安全）
 */
export function useCategories() {
  const { status, error } = useSchema();
  const categories = useSchemaStore((s) => s.getAllCategories());

  return {
    categories: status === LoadStatus.Ready ? categories : [],
    status,
    error,
  };
}

/**
 * 获取可见分类（安全）
 */
export function useVisibleCategories() {
  const { status, error } = useSchema();
  const categories = useSchemaStore((s) => s.getVisibleCategories());

  return {
    categories: status === LoadStatus.Ready ? categories : [],
    status,
    error,
  };
}

/**
 * 获取所有变量类型（安全）
 */
export function useVariableTypes() {
  const { status, error } = useSchema();
  const variableTypes = useSchemaStore((s) => s.getAllVariableTypes());

  return {
    variableTypes: status === LoadStatus.Ready ? variableTypes : [],
    status,
    error,
  };
}

/**
 * 获取所有 UI 样式（安全）
 */
export function useUIStyles() {
  const { status, error } = useSchema();
  const uiStyles = useSchemaStore((s) => s.getAllUIStyles());

  return {
    uiStyles: status === LoadStatus.Ready ? uiStyles : [],
    status,
    error,
  };
}
