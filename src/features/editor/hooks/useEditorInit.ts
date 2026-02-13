/**
 * 编辑器初始化 Hook
 *
 * 负责在编辑器启动时加载所有必要的数据：
 * - Schema 定义（Pin 类型、分类、样式等）
 * - 节点定义
 */

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSchemaStore } from "@/features/shema";
import type { NodeDefinition } from "@/shared/types/editor";

interface EditorInitState {
  isInitialized: boolean;
  isLoading: boolean;
  error: string | null;
  nodeDefinitions: NodeDefinition[];
}

/**
 * 编辑器初始化 Hook
 *
 * 使用示例：
 * ```tsx
 * function Editor() {
 *   const { isInitialized, isLoading, error, nodeDefinitions } = useEditorInit();
 *
 *   if (isLoading) return <LoadingScreen />;
 *   if (error) return <ErrorScreen error={error} />;
 *   if (!isInitialized) return null;
 *
 *   return <EditorContent nodeDefinitions={nodeDefinitions} />;
 * }
 * ```
 */
export function useEditorInit(): EditorInitState {
  const [state, setState] = useState<EditorInitState>({
    isInitialized: false,
    isLoading: true,
    error: null,
    nodeDefinitions: [],
  });

  const loadSchema = useSchemaStore((s) => s.loadSchema);
  const schemaLoaded = useSchemaStore((s) => s.isLoaded);
  const schemaError = useSchemaStore((s) => s.error);

  useEffect(() => {
    let cancelled = false;

    async function initialize() {
      try {
        // 1. 加载 Schema（如果尚未加载）
        if (!schemaLoaded) {
          await loadSchema();
        }

        // 2. 加载节点定义
        const nodeDefs: NodeDefinition[] = await invoke("get_node_definitions");

        if (cancelled) return;

        setState({
          isInitialized: true,
          isLoading: false,
          error: null,
          nodeDefinitions: nodeDefs,
        });

        console.log("[EditorInit] Initialization complete", {
          nodeDefinitions: nodeDefs.length,
        });
      } catch (err) {
        if (cancelled) return;

        const errorMessage = err instanceof Error ? err.message : String(err);
        console.error("[EditorInit] Initialization failed:", errorMessage);

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

  // 处理 schema 加载错误
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

/**
 * 确保编辑器已初始化
 * 如果未初始化，会抛出错误
 */
export function useRequireEditorInit() {
  const schemaLoaded = useSchemaStore((s) => s.isLoaded);

  if (!schemaLoaded) {
    throw new Error(
      "Editor not initialized. Make sure to use useEditorInit() at the app root."
    );
  }
}

export default useEditorInit;
