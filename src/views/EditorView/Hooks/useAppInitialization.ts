import { useState, useEffect, useRef } from 'react';
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

  // 使用 ref 防止重复初始化项目同步
  const projectSyncedRef = useRef(false);

  const isSchemaLoaded = useSchemaStore((s) => s.isLoaded);
  const loadSchema = useSchemaStore((s) => s.loadSchema);
  const { isInitialized: isRegistryInitialized, error: registryError } = useNodeRegistry();

  // 步骤 1: 加载 Schema
  useEffect(() => {
    if (!isSchemaLoaded) {
      console.log('[useAppInitialization] Loading schema...');
      loadSchema().catch(err => {
        console.error('[useAppInitialization] Failed to load schema:', err);
        setState({ isInitialized: false, isLoading: false, error: String(err) });
      });
    }
  }, [isSchemaLoaded, loadSchema]);

  // 步骤 2 & 3: 当 Schema 和 Registry 都准备好后，同步项目状态
  useEffect(() => {
    // 如果已经出错，不继续
    if (state.error) return;

    // 如果 Schema 或 Registry 还未准备好，等待
    if (!isSchemaLoaded || !isRegistryInitialized) {
      return;
    }

    // 如果 Registry 初始化出错
    if (registryError) {
      setState({ isInitialized: false, isLoading: false, error: registryError });
      return;
    }

    // 如果已经同步过项目，直接标记完成
    if (projectSyncedRef.current) {
      setState({ isInitialized: true, isLoading: false, error: null });
      return;
    }

    // 同步项目状态
    const syncProject = async () => {
      try {
        console.log('[useAppInitialization] Syncing project state from backend...');
        const projectData = await initProjectSync();

        if (projectData) {
          console.log('[useAppInitialization] Project state restored from backend:', {
            events: Object.keys(projectData.events).length,
            functions: Object.keys(projectData.functions).length,
            macros: Object.keys(projectData.macros).length,
          });
        } else {
          console.log('[useAppInitialization] No project data in backend');
        }

        projectSyncedRef.current = true;
        console.log('[useAppInitialization] Application initialization complete');
        setState({ isInitialized: true, isLoading: false, error: null });
      } catch (error) {
        console.error('[useAppInitialization] Failed to sync project:', error);
        setState({
          isInitialized: false,
          isLoading: false,
          error: error instanceof Error ? error.message : String(error),
        });
      }
    };

    syncProject();
  }, [isSchemaLoaded, isRegistryInitialized, registryError, state.error]);

  return state;
}
