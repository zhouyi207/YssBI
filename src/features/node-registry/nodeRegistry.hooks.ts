/// hooks —— 生命周期 + 组合逻辑（重点）

import { useEffect } from "react";
import { useNodeRegistryStore } from "./nodeRegistry.store";
import { NodeRegistryState } from "./nodeRegistry.types";
import { LoadStatus } from "@/shared/types/loadStatus";

/**
 * NodeRegistry 初始化 Hook
 *
 * 语义：
 * - 首次使用时自动触发初始化
 * - 返回标准的 NodeRegistryState（status + error）
 */
export function useNodeRegistry(): NodeRegistryState {
  const status = useNodeRegistryStore((s) => s.status);
  const error = useNodeRegistryStore((s) => s.error);
  const syncFromBackend = useNodeRegistryStore((s) => s.syncFromBackend);

  useEffect(() => {
    if (status === LoadStatus.Idle) {
      syncFromBackend().catch((err) => {
        console.error("[useNodeRegistry] Initialization failed:", err);
      });
    }
  }, [status, syncFromBackend]);

  return {
    status,
    error,
  };
}

/**
 * 获取所有节点定义（安全）
 *
 * - 未 Ready 时返回空数组
 * - 调用方无需关心初始化时序
 * - 返回缓存的数组引用，避免触发不必要的重渲染
 */
export function useNodeDefinitions() {
  const { status, error } = useNodeRegistry();
  const definitions = useNodeRegistryStore((s) => s.definitionsArray);

  return {
    definitions: status === LoadStatus.Ready ? definitions : [],
    status,
    error,
  };
}
