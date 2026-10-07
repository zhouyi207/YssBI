import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { pluginService } from "@/services/plugins/pluginService";
import type {
  PluginDiagnostic,
  PluginStorageUsage,
  TaskSnapshot,
} from "@/shared/types/plugins/generated";
import type { PluginSnapshot } from "./pluginRegistry";

interface MaintenanceSnapshot {
  tasks: TaskSnapshot[];
  cursor: string | null;
  storage: PluginStorageUsage | null;
  diagnostics: PluginDiagnostic[];
}
const emptySnapshot = (): MaintenanceSnapshot => ({
  tasks: [],
  cursor: null,
  storage: null,
  diagnostics: [],
});

async function readMaintenance(pluginId: string): Promise<MaintenanceSnapshot> {
  const [history, storage, diagnostics] = await Promise.all([
    pluginService.history(pluginId),
    pluginService.storage(pluginId),
    pluginService.diagnostics(pluginId),
  ]);
  return { tasks: history.tasks, cursor: history.nextCursor ?? null, storage, diagnostics };
}

export function usePluginMaintenance(plugin: PluginSnapshot) {
  const pluginId = plugin.manifest.id;
  const installationGeneration = plugin.installationGeneration;
  const identity = useMemo(
    () => ({ pluginId, installationGeneration }),
    [pluginId, installationGeneration],
  );
  const owner = useRef<typeof identity | null>(null);
  const pending = useRef<object | null>(null);
  const [snapshot, setSnapshot] = useState(emptySnapshot);
  const [status, setStatus] = useState({ busy: false, error: false });

  const run = useCallback(
    async (action: (isCurrent: () => boolean) => Promise<void>) => {
      if (owner.current !== identity || pending.current) return;
      const operation = {};
      pending.current = operation;
      const isCurrent = () => owner.current === identity && pending.current === operation;
      setStatus({ busy: true, error: false });
      let failed = false;
      try {
        await action(isCurrent);
      } catch {
        failed = true;
      } finally {
        if (isCurrent()) {
          pending.current = null;
          setStatus({ busy: false, error: failed });
        }
      }
    },
    [identity],
  );

  const refresh = useCallback(
    () =>
      run(async (isCurrent) => {
        const next = await readMaintenance(pluginId);
        if (isCurrent()) setSnapshot(next);
      }),
    [pluginId, run],
  );

  useEffect(() => {
    owner.current = identity;
    pending.current = null;
    setSnapshot(emptySnapshot());
    void refresh();
    return () => {
      owner.current = null;
      pending.current = null;
    };
  }, [identity, refresh]);

  const more = async () => {
    if (!snapshot.cursor) return;
    await run(async (isCurrent) => {
      const page = await pluginService.history(pluginId, snapshot.cursor);
      if (isCurrent())
        setSnapshot((previous) => ({
          ...previous,
          tasks: [...previous.tasks, ...page.tasks],
          cursor: page.nextCursor ?? null,
        }));
    });
  };
  const change = (operation: (id: string) => Promise<unknown>) =>
    run(async (isCurrent) => {
      await operation(pluginId);
      if (!isCurrent()) return;
      const next = await readMaintenance(pluginId);
      if (isCurrent()) setSnapshot(next);
    });

  return {
    ...snapshot,
    ...status,
    refresh,
    more,
    clearHistory: () => change(pluginService.clearHistory),
    clearCache: () => change(pluginService.clearCache),
    collectGarbage: () => change(() => pluginService.collectGarbage()),
  };
}
