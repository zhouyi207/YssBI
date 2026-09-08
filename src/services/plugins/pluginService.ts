import { invokeCommand } from "@/services/ipc";
import schema from "@/shared/types/plugins/schema.json";
import type {
  InstalledPlugin,
  PackageInspection,
  ViewSession,
  TaskSnapshot,
} from "@/shared/types/plugins/generated";

type Schema = {
  $ref?: string;
  type?: string | string[];
  anyOf?: Schema[];
  oneOf?: Schema[];
  enum?: unknown[];
  required?: string[];
  properties?: Record<string, Schema | boolean>;
  additionalProperties?: boolean;
  items?: Schema;
  minimum?: number;
  maximum?: number;
};
function valid(value: unknown, shape: Schema | boolean, depth = 0): boolean {
  if (depth > 64 || shape === false) return false;
  if (shape === true) return true;
  if (shape.$ref)
    return valid(
      value,
      (schema.$defs as Record<string, Schema>)[shape.$ref.split("/").pop()!],
      depth + 1,
    );
  if (shape.anyOf || shape.oneOf)
    return (shape.anyOf ?? shape.oneOf)!.some((item) => valid(value, item, depth + 1));
  if (Array.isArray(shape.type))
    return shape.type.some((type) => valid(value, { ...shape, type }, depth + 1));
  if (shape.enum && !shape.enum.includes(value)) return false;
  switch (shape.type) {
    case "null":
      return value === null;
    case "string":
      return typeof value === "string";
    case "boolean":
      return typeof value === "boolean";
    case "integer":
    case "number":
      return (
        typeof value === "number" &&
        Number.isFinite(value) &&
        (shape.type !== "integer" || Number.isSafeInteger(value)) &&
        (shape.minimum === undefined || value >= shape.minimum) &&
        (shape.maximum === undefined || value <= shape.maximum)
      );
    case "array":
      return (
        Array.isArray(value) &&
        value.length <= 4096 &&
        value.every((item) => valid(item, shape.items ?? true, depth + 1))
      );
    case "object": {
      if (!value || typeof value !== "object" || Array.isArray(value)) return false;
      const object = value as Record<string, unknown>;
      return (
        (shape.required ?? []).every((key) => Object.prototype.hasOwnProperty.call(object, key)) &&
        Object.entries(object).every(([key, item]) =>
          shape.properties?.[key] !== undefined
            ? valid(item, shape.properties[key], depth + 1)
            : shape.additionalProperties !== false,
        )
      );
    }
    default:
      return true;
  }
}
function parse<T>(name: keyof typeof schema.$defs, value: unknown): T {
  if (!valid(value, schema.$defs[name] as Schema)) throw new Error("plugin_wire_invalid");
  return value as T;
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
  async install(path: string, digest: string): Promise<InstalledPlugin> {
    return parse(
      "InstalledPlugin",
      await invokeCommand("install_plugin_package", {
        path,
        expectedDigest: digest,
        operationId: `op-${crypto.randomUUID()}`,
        approveNative: true,
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
  grantExport(sessionId: string, path: string) {
    return invokeCommand<string>("grant_plugin_export", { sessionId, path });
  },
  call(sessionId: string, method: string, input: unknown) {
    return invokeCommand<unknown>("call_plugin_view", { sessionId, method, input });
  },
  async tasks(): Promise<TaskSnapshot[]> {
    const result = await invokeCommand<unknown>("list_plugin_tasks");
    if (!Array.isArray(result)) throw new Error("plugin_wire_invalid");
    return result.map((task) => parse("TaskSnapshot", task));
  },
};
