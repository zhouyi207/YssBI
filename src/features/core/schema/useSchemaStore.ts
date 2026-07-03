/// store —— 只负责「状态 + backend 同步」
/// Schema 在初始化时加载，含 nodeDefinitions（含 pin metaData 如 dropdown 的 widget_options）

import { create } from "zustand";
import { SchemaService } from "@/services/schema";
import { LoadStatus } from "@/shared/types/ui";
import { SchemaState } from "@/shared/types/state";
import type { NodeDefinition, TypeSystemSnapshot } from "@/shared/types/domain";
import { EMPTY_TYPE_SYSTEM, setActiveTypeSystem } from "@/shared/types/domain";
import { logger } from '@/utils/appLogger';
import { useNodeRegistryStore } from "@/features/core/nodeRegister/useNodeRegistryStore";

interface SchemaStore extends SchemaState {
  // Schema 数据（含 nodeDefinitions，用于 pin dropdown 等）
  status: LoadStatus;
  error: string | null;
  nodeDefinitions: NodeDefinition[];
  typeSystem: TypeSystemSnapshot;

  // 操作
  syncFromBackend: () => Promise<void>;
  clear: () => void;
}

export const useSchemaStore = create<SchemaStore>((set, get) => ({
  // data
  status: LoadStatus.Idle,
  error: null,
  nodeDefinitions: [],
  typeSystem: EMPTY_TYPE_SYSTEM,

  syncFromBackend: async () => {
    const { status } = get();

    // 幂等保护
    if (status === LoadStatus.Loading || status === LoadStatus.Ready) {
      logger.sys.debug('Already loading or loaded, skipping...', 'Schema');
      return;
    }

    const startTime = performance.now();
    logger.sys.debug('Loading schema from backend...', 'Schema');

    set({ status: LoadStatus.Loading, error: null });

    try {
      const schema = await SchemaService.getEditorSchema();

      const definitions = new Map<string, NodeDefinition>();
      (schema.nodeDefinitions ?? []).forEach((def) => {
        definitions.set(def.nodeType, def);
      });

      // 同步到 Node Registry，供节点渲染使用
      useNodeRegistryStore.getState().setDefinitionsFromSchema(definitions);
      setActiveTypeSystem(schema.typeSystem);

      set({
        status: LoadStatus.Ready,
        nodeDefinitions: schema.nodeDefinitions ?? [],
        typeSystem: schema.typeSystem,
      });

      const duration = performance.now() - startTime;
      logger.sys.info(`Schema loaded successfully, nodeTypes: ${definitions.size}, duration: ${duration.toFixed(0)}ms`, 'Schema');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      logger.sys.error('Failed to load schema: ' + errorMessage, 'Schema');

      set({
        status: LoadStatus.Error,
        error: errorMessage,
      });

      throw err;
    }
  },

  clear: () => {
    setActiveTypeSystem(EMPTY_TYPE_SYSTEM);
    set({
      status: LoadStatus.Idle,
      error: null,
      nodeDefinitions: [],
      typeSystem: EMPTY_TYPE_SYSTEM,
    });
  },

}));
