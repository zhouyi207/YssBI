/**
 * 编辑器初始化 Hook
 * 负责加载 Schema 和节点定义
 */

import { useEffect, useState } from 'react';
import { LoadStatus } from '@/shared/types/ui';
import { useSchemaStore } from '@/features/core/schema';
import { SchemaService } from '@/services/schema';
import type { NodeDefinition } from '@/shared/types/domain';

export interface EditorInitState {
  isInitialized: boolean;
  isLoading: boolean;
  error: string | null;
  nodeDefinitions: NodeDefinition[];
}

export function useEditorInit(): EditorInitState {
  const [state, setState] = useState<EditorInitState>({
    isInitialized: false,
    isLoading: true,
    error: null,
    nodeDefinitions: [],
  });

  const loadSchema = useSchemaStore((s) => s.syncFromBackend);
  const schemaLoaded = useSchemaStore((s) => s.status === LoadStatus.Ready);
  const schemaError = useSchemaStore((s) => s.error);

  useEffect(() => {
    let cancelled = false;

    async function initialize() {
      try {
        if (!schemaLoaded) {
          await loadSchema();
        }

        const nodeDefs = await SchemaService.getNodeDefinition();

        if (cancelled) return;

        setState({
          isInitialized: true,
          isLoading: false,
          error: null,
          nodeDefinitions: nodeDefs,
        });

        console.log('[EditorInit] Initialization complete', { nodeDefinitions: nodeDefs.length });
      } catch (err) {
        if (cancelled) return;

        const errorMessage = err instanceof Error ? err.message : String(err);
        console.error('[EditorInit] Initialization failed:', errorMessage);

        setState({
          isInitialized: false,
          isLoading: false,
          error: errorMessage,
          nodeDefinitions: [],
        });
      }
    }

    initialize();

    return () => {
      cancelled = true;
    };
  }, [loadSchema, schemaLoaded]);

  useEffect(() => {
    if (schemaError) {
      setState((prev) => ({
        ...prev,
        error: prev.error || schemaError,
      }));
    }
  }, [schemaError]);

  return state;
}

export function useRequireEditorInit() {
  const schemaLoaded = useSchemaStore((s) => s.status === LoadStatus.Ready);

  if (!schemaLoaded) {
    throw new Error('Editor not initialized. Make sure to use useEditorInit() at the app root.');
  }
}
