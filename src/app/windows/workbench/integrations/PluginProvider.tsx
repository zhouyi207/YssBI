import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import type { InstalledPlugin, PluginView } from "@/shared/types/plugins/generated";
import { pluginService } from "@/services/plugins/pluginService";
import { installLocalPlugin, uninstallPlugin } from "@/features/application/plugins/pluginActions";
import { openPluginWorkbenchView, syncPluginWorkbenchViews } from "@/modules/workbench/public";

interface PluginContextValue {
  plugins: InstalledPlugin[];
  loading: boolean;
  busy: boolean;
  error: string | null;
  refresh(): Promise<void>;
  install(): Promise<void>;
  uninstall(plugin: InstalledPlugin): Promise<void>;
  setEnabled(plugin: InstalledPlugin): Promise<void>;
  open(pluginId: string, view: PluginView): void;
}
const PluginContext = createContext<PluginContextValue | null>(null);
export function PluginProvider({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const [plugins, setPlugins] = useState<InstalledPlugin[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const snapshot = useRef<InstalledPlugin[] | null>(null);
  const refreshEpoch = useRef(0);
  const mounted = useRef(true);
  const running = useRef(false);
  const publishSnapshot = useCallback(async (next: InstalledPlugin[]) => {
    if (!mounted.current) return;
    snapshot.current = next;
    setPlugins(next);
    await syncPluginWorkbenchViews(
      next.map((plugin) => plugin.manifest.id),
      next
        .filter((plugin) => plugin.enabled)
        .flatMap((plugin) =>
          plugin.manifest.contributes.views
            .filter((view) => view.location === "sidebar")
            .map((view) => ({
              pluginId: plugin.manifest.id,
              viewId: view.id,
              title: view.title,
              location: view.location,
            })),
        ),
      () => mounted.current && snapshot.current === next,
    );
  }, []);
  const refresh = useCallback(async () => {
    const epoch = ++refreshEpoch.current;
    try {
      const next = await pluginService.list();
      if (!mounted.current || epoch !== refreshEpoch.current) return;
      await publishSnapshot(next);
      if (epoch === refreshEpoch.current) setError(null);
    } catch {
      if (mounted.current && epoch === refreshEpoch.current) setError(t("plugins.loadFailed"));
    } finally {
      if (mounted.current && epoch === refreshEpoch.current) setLoading(false);
    }
  }, [t, publishSnapshot]);
  useEffect(() => {
    mounted.current = true;
    void refresh();
    return () => {
      mounted.current = false;
      refreshEpoch.current += 1;
      snapshot.current = null;
    };
  }, [refresh]);
  const run = async (action: () => Promise<unknown>) => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setError(null);
    try {
      await action();
      await refresh();
    } catch {
      if (mounted.current) setError(t("plugins.operationFailed"));
    } finally {
      running.current = false;
      if (mounted.current) setBusy(false);
    }
  };
  return (
    <PluginContext.Provider
      value={{
        plugins,
        loading,
        busy,
        error,
        refresh,
        install: () => run(() => installLocalPlugin(t)),
        uninstall: (plugin) =>
          run(async () => {
            if (!(await uninstallPlugin(plugin.manifest.id, plugin.manifest.name, t))) return;
            // A confirmed backend removal remains authoritative even if the next list query fails.
            refreshEpoch.current += 1;
            await publishSnapshot(
              (snapshot.current ?? []).filter((entry) => entry.manifest.id !== plugin.manifest.id),
            );
          }),
        setEnabled: (plugin) =>
          run(() => pluginService.setEnabled(plugin.manifest.id, !plugin.enabled)),
        open: (pluginId, view) => {
          const owner = snapshot.current?.find(
            (plugin) => plugin.manifest.id === pluginId && plugin.enabled,
          );
          const contribution = owner?.manifest.contributes.views.find(
            (entry) => entry.id === view.id,
          );
          if (!owner || !contribution) return;
          void openPluginWorkbenchView(
            {
              pluginId,
              viewId: contribution.id,
              title: contribution.title,
              location: contribution.location,
            },
            true,
            () =>
              mounted.current &&
              !!snapshot.current?.some(
                (plugin) =>
                  plugin.manifest.id === pluginId &&
                  plugin.enabled &&
                  plugin.installationGeneration === owner.installationGeneration,
              ),
          );
        },
      }}
    >
      {children}
    </PluginContext.Provider>
  );
}
export function usePlugins() {
  const value = useContext(PluginContext);
  if (!value) throw new Error("PluginProvider is required");
  return value;
}
