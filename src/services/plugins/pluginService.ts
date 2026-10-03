import { invokeCommand } from "@/services/ipc";
import {
  parsePluginWire as parse,
  parsePluginViewReply,
  pluginExportGrantSchema,
  pluginGarbageCountSchema,
} from "./pluginWireParser";
import type {
  InstalledPlugin,
  PackageInspection,
  ViewSession,
  TaskSnapshot,
  TaskHistoryPage,
  PluginStorageUsage,
  PluginDiagnostic,
} from "@/shared/types/plugins/generated";

function requirePluginIdentity<T extends { pluginId: string }>(value: T, pluginId: string): T {
  if (value.pluginId !== pluginId) throw new Error("plugin_wire_invalid");
  return value;
}

export const pluginService = {
  async list(): Promise<InstalledPlugin[]> {
    const value = await invokeCommand<unknown>("list_plugins");
    if (!Array.isArray(value)) throw new Error("plugin_wire_invalid");
    return value.map((item) => parse("InstalledPlugin", item));
  },
  async inspect(path: string): Promise<PackageInspection> {
    return parse("PackageInspection", await invokeCommand("inspect_plugin_package", { path }));
  },
  async install(
    path: string,
    digest: string,
    operationId: string,
    approvedPreviousSigner: string | null,
  ): Promise<InstalledPlugin> {
    return parse(
      "InstalledPlugin",
      await invokeCommand("install_plugin_package", {
        path,
        expectedDigest: digest,
        operationId,
        approveNative: true,
        approvedPreviousSigner,
      }),
    );
  },
  setEnabled(pluginId: string, enabled: boolean) {
    return invokeCommand<void>("set_plugin_enabled", { pluginId, enabled });
  },
  uninstall(pluginId: string) {
    return invokeCommand<void>("uninstall_plugin", { pluginId });
  },
  async attach(pluginId: string, viewId: string): Promise<ViewSession> {
    return parse("ViewSession", await invokeCommand("attach_plugin_view", { pluginId, viewId }));
  },
  detach(sessionId: string) {
    return invokeCommand<void>("detach_plugin_view", { sessionId });
  },
  async grantExport(sessionId: string, path: string) {
    return pluginExportGrantSchema.parse(
      await invokeCommand<unknown>("grant_plugin_export", { sessionId, path }),
    );
  },
  async call(sessionId: string, method: string, input: unknown) {
    return parsePluginViewReply(
      method,
      await invokeCommand<unknown>("call_plugin_view", { sessionId, method, input }),
    );
  },
  async tasks(): Promise<TaskSnapshot[]> {
    const result = await invokeCommand<unknown>("list_plugin_tasks");
    if (!Array.isArray(result)) throw new Error("plugin_wire_invalid");
    return result.map((task) => parse("TaskSnapshot", task));
  },
  async history(pluginId: string, cursor: string | null = null): Promise<TaskHistoryPage> {
    const page = parse<TaskHistoryPage>(
      "TaskHistoryPage",
      await invokeCommand("get_plugin_task_history", { pluginId, cursor, limit: 25 }),
    );
    page.tasks.forEach((task) => requirePluginIdentity(task, pluginId));
    return page;
  },
  clearHistory(pluginId: string) {
    return invokeCommand<void>("clear_plugin_task_history", { pluginId });
  },
  async storage(pluginId: string): Promise<PluginStorageUsage> {
    return requirePluginIdentity(
      parse<PluginStorageUsage>(
        "PluginStorageUsage",
        await invokeCommand("get_plugin_storage_usage", { pluginId }),
      ),
      pluginId,
    );
  },
  async clearCache(pluginId: string): Promise<PluginStorageUsage> {
    return requirePluginIdentity(
      parse<PluginStorageUsage>(
        "PluginStorageUsage",
        await invokeCommand("clear_plugin_cache", { pluginId }),
      ),
      pluginId,
    );
  },
  async collectGarbage() {
    return pluginGarbageCountSchema.parse(await invokeCommand<unknown>("collect_plugin_garbage"));
  },
  async diagnostics(pluginId: string): Promise<PluginDiagnostic[]> {
    const result = await invokeCommand<unknown>("get_plugin_diagnostics", { pluginId });
    if (!Array.isArray(result)) throw new Error("plugin_wire_invalid");
    return result.map((item) =>
      requirePluginIdentity(parse<PluginDiagnostic>("PluginDiagnostic", item), pluginId),
    );
  },
};

/** Create once per logical operation; keep the returned value for transport retries. */
export function createOperationId(): string {
  return `op-${Date.now()}-${crypto.randomUUID()}`;
}
