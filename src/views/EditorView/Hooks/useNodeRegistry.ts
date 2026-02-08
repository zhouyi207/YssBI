import { useEffect, useState } from 'react';
import { BaseNode } from '../Types/nodes';
import { Position } from '../../../shared/types';
import { useNodeRegistryStore } from '../Store/useNodeRegistryStore';

/**
 * 节点注册表初始化状态
 */
interface NodeRegistryState {
  isInitialized: boolean;
  isLoading: boolean;
  error: string | null;
}

/**
 * 节点注册表 Hook
 * 
 * 管理 NodeRegistry 的初始化和状态
 * 提供统一的接口来访问节点定义
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

    const initialize = async () => {
      try {
        await syncFromBackend();
        setHasInitialized(true);
      } catch (error) {
        console.error('[useNodeRegistry] Failed to initialize:', error);
      }
    };

    initialize();
  }, [isInitialized, syncFromBackend]);

  return {
    isInitialized: hasInitialized,
    isLoading,
    error,
  };
}

/**
 * 获取所有节点定义的 Hook
 */
export function useNodeDefinitions() {
  const { isInitialized, isLoading, error } = useNodeRegistry();
  const getAllDefinitions = useNodeRegistryStore((s) => s.getAllDefinitions);

  return {
    definitions: isInitialized ? getAllDefinitions() : [],
    isInitialized,
    isLoading,
    error,
  };
}

/**
 * 根据类型创建一个新的节点实例
 */
export function createNode(type: string, id: string, position: Position): BaseNode | null {
  const def = useNodeRegistryStore.getState().getDefinition(type);
  if (!def) {
    console.error(`Node type ${type} not found in registry`);
    return null;
  }
  return new BaseNode(id, def, position);
}

/**
 * 获取节点定义
 */
export function getNodeDefinition(type: string) {
  return useNodeRegistryStore.getState().getDefinition(type);
}

/**
 * 检查节点类型是否存在
 */
export function hasNodeDefinition(type: string): boolean {
  return useNodeRegistryStore.getState().hasDefinition(type);
}
