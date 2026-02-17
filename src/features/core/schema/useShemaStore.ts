/// store —— 只负责「状态 + backend 同步」

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { LoadStatus } from "@/shared/types/ui";
import { SchemaState } from "@/shared/types/state";

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
      console.log('[Schema] Already loading or loaded, skipping...');
      return;
    }

    const startTime = performance.now();
    console.log('[Schema] Loading schema from backend...');

    set({ status: LoadStatus.Loading, error: null });

    try {
      await invoke("get_editor_schema_command");

      set({
        status: LoadStatus.Ready,
      });

      const duration = performance.now() - startTime;
      console.log('[Schema] ✓ Schema loaded successfully', {
        duration: `${duration.toFixed(0)}ms`,
      });
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[Schema] ✗ Failed to load schema:', errorMessage);

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
