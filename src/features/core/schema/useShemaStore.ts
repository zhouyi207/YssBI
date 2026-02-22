/// store —— 只负责「状态 + backend 同步」

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { LoadStatus } from "@/shared/types/ui";
import { SchemaState } from "@/shared/types/state";
import { logger } from '@/utils/appLogger';

interface SchemaStore extends SchemaState {
  // Schema 数据
  status: LoadStatus;
  error: string | null;
  
  // 操作
  syncFromBackend: () => Promise<void>;
  clear: () => void;
}

export const useSchemaStore = create<SchemaStore>((set, get) => ({
  // data
  status: LoadStatus.Idle,
  error: null,

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
      await invoke("get_editor_schema_command");

      set({
        status: LoadStatus.Ready,
      });

      const duration = performance.now() - startTime;
      logger.sys.info(`Schema loaded successfully, duration: ${duration.toFixed(0)}ms`, 'Schema');
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

  clear: () =>
    set({
      status: LoadStatus.Idle,
      error: null,
    }),

}));
