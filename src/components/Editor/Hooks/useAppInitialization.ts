import { useState, useEffect } from 'react';
import { useNodeRegistry } from './useNodeRegistry';
import { useSchemaStore } from '../Store/useSchemaStore';
import { initProjectSync } from './useProjectSync';

/**
 * 应用初始化状态
 */
interface InitializationState {
  isInitialized: boolean;
  isLoading: boolean;
  error: string | null;
}

/**
 * 应用初始化 Hook
 * 
 * 统一管理应用启动时的所有初始化逻辑：
 * 1. 加载 Schema
 * 2. 同步节点定义
 * 3. 同步项目状态
 */
export function useAppInitialization(): InitializationState {
  const [state, setState] = useState<InitializationState>({
    isInitialized: false,
    isLoading: true,
    error: null,
  });

  const isSchemaLoaded = useSchemaStore((s) => s.isLoaded);
  const loadSchema = useSchemaStore((s) => s.loadSchema);
  const { isInitialized: isRegistryInitialized, isLoading: isRegistryLoading, error: registryError } = useNodeRegistry();

  useEffect(() => {
    const initialize = async () => {
      console.log('[useAppInitialization] Starting application initialization...');
      setState(prev => ({ ...prev, isLoading: true, error: null }));

      try {
        // 步骤 1: 加载 Schema（如果尚未加载）
        if (!isSchemaLoaded) {
          console.log('[useAppInitialization] Loading schema...');
          await loadSchema();
          console.log('[useAppInitialization] Schema loaded successfully');
        }

        // 步骤 2: 等待节点注册表初始化完成（由 useNodeRegistry 处理）
        if (!isRegistryInitialized && !isRegistryLoading && !registryError) {
          console.log('[useAppInitialization] Waiting for node registry initialization...');
          return;
        }

        if (registryError) {
          throw new Error(`Node registry initialization failed: ${registryError}`);
        }

        // 步骤 3: 同步项目状态
        console.log('[useAppInitialization] Syncing project state from backend...');
        const projectData = await initProjectSync();
        
        if (projectData) {
          console.log('[useAppInitialization] Project state restored from backend:', {
            events: Object.keys(projectData.events),
            functions: Object.keys(projectData.functions),
            macros: Object.keys(projectData.macros),
          });
        } else {
          console.log('[useAppInitialization] No project data in backend');
        }

        console.log('[useAppInitialization] Application initialization complete');
        setState({
          isInitialized: true,
          isLoading: false,
          error: null,
        });
      } catch (error) {
        console.error('[useAppInitialization] Failed to initialize application:', error);
        setState({
          isInitialized: false,
          isLoading: false,
          error: error instanceof Error ? error.message : String(error),
        });
      }
    };

    initialize();
  }, [isSchemaLoaded, loadSchema, isRegistryInitialized, isRegistryLoading, registryError]);

  return state;
}
