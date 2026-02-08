/// hooks —— 生命周期 + 组合逻辑（重点）

import { useEffect, useState } from "react";
import { useNodeRegistryStore } from "./nodeRegistry.store";
import { NodeRegistryState } from "./nodeRegistry.types";

/**
 * NodeRegistry 初始化 Hook
 */
export function useNodeRegistry(): NodeRegistryState {
  const isInitialized = useNodeRegistryStore((s) => s.isInitialized);
  const isLoading = useNodeRegistryStore((s) => s.isLoading);
  const error = useNodeRegistryStore((s) => s.error);
  const syncFromBackend = useNodeRegistryStore((s) => s.syncFromBackend);

  const [hasInitialized, setHasInitialized] = useState(isInitialized);

  useEffect(() => {
    if (isInitialized) {
      setHasInitialized(true);
      return;
    }

    syncFromBackend().then(
      () => setHasInitialized(true),
      (err) => console.error("[useNodeRegistry]", err),
    );
  }, [isInitialized, syncFromBackend]);

  return {
    isInitialized: hasInitialized,
    isLoading,
    error,
  };
}

/**
 * 获取所有节点定义（安全）
 */
export function useNodeDefinitions() {
  const { isInitialized, isLoading, error } = useNodeRegistry();
  const definitions = useNodeRegistryStore((s) => s.getAllDefinitions());

  return {
    definitions: isInitialized ? definitions : [],
    isInitialized,
    isLoading,
    error,
  };
}
