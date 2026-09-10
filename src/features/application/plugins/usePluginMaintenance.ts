import { useCallback, useEffect, useRef, useState } from "react";
import { pluginService } from "@/services/plugins/pluginService";
import type {
  InstalledPlugin,
  PluginDiagnostic,
  PluginStorageUsage,
  TaskSnapshot,
} from "@/shared/types/plugins/generated";

export function usePluginMaintenance(plugin: InstalledPlugin | null) {
  const [tasks, setTasks] = useState<TaskSnapshot[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [storage, setStorage] = useState<PluginStorageUsage | null>(null);
  const [diagnostics, setDiagnostics] = useState<PluginDiagnostic[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const generation = useRef(0);
  const refresh = useCallback(async () => {
    const identity = ++generation.current;
    if (!plugin) return;
    setBusy(true);
    setError(false);
    try {
      const [history, usage, output] = await Promise.all([
        pluginService.history(plugin.manifest.id),
        pluginService.storage(plugin.manifest.id),
        pluginService.diagnostics(plugin.manifest.id),
      ]);
      if (identity !== generation.current) return;
      setTasks(history.tasks);
      setCursor(history.nextCursor ?? null);
      setStorage(usage);
      setDiagnostics(output);
    } catch {
      if (identity === generation.current) setError(true);
    } finally {
      if (identity === generation.current) setBusy(false);
    }
  }, [plugin]);
  useEffect(() => {
    setTasks([]);
    setCursor(null);
    setStorage(null);
    setDiagnostics([]);
    void refresh();
    return () => {
      generation.current += 1;
    };
  }, [refresh]);
  const more = async () => {
    if (!plugin || !cursor || busy) return;
    const identity = ++generation.current;
    setBusy(true);
    try {
      const page = await pluginService.history(plugin.manifest.id, cursor);
      if (identity !== generation.current) return;
      setTasks((previous) => [...previous, ...page.tasks]);
      setCursor(page.nextCursor ?? null);
    } catch {
      if (identity === generation.current) setError(true);
    } finally {
      if (identity === generation.current) setBusy(false);
    }
  };
  const change = async (operation: (id: string) => Promise<unknown>) => {
    if (!plugin || busy) return;
    const identity = ++generation.current;
    setBusy(true);
    setError(false);
    try {
      await operation(plugin.manifest.id);
      if (identity === generation.current) await refresh();
    } catch {
      if (identity === generation.current) setError(true);
    } finally {
      if (identity === generation.current) setBusy(false);
    }
  };
  return {
    tasks,
    cursor,
    storage,
    diagnostics,
    busy,
    error,
    refresh,
    more,
    clearHistory: () => change(pluginService.clearHistory),
    clearCache: () => change(pluginService.clearCache),
    collectGarbage: () => change(() => pluginService.collectGarbage()),
  };
}
