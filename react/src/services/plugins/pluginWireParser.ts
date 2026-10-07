import { z } from "zod";
import schema from "@/shared/types/plugins/schema.json";
import type { PluginView } from "@/shared/types/plugins/generated";

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
          shape.properties && Object.prototype.hasOwnProperty.call(shape.properties, key)
            ? valid(item, shape.properties[key], depth + 1)
            : shape.additionalProperties !== false,
        )
      );
    }
    default:
      return true;
  }
}
export function parsePluginWire<T>(name: keyof typeof schema.$defs, value: unknown): T {
  if (!valid(value, schema.$defs[name] as Schema)) throw new Error("plugin_wire_invalid");
  return value as T;
}

export const pluginExportGrantSchema = z.string().min(1);
export const pluginGarbageCountSchema = z.number().int().nonnegative();

const revealReplySchema = z.strictObject({
  hostUi: z.strictObject({ kind: z.literal("reveal"), path: z.string().min(1) }),
});
const saveFileReplySchema = z.strictObject({
  hostUi: z.strictObject({
    kind: z.literal("saveFile"),
    options: z.object({ defaultPath: z.string().nullish() }).nullish(),
  }),
});
const openViewReplySchema = z.strictObject({
  openView: z.strictObject({
    pluginId: z.string().min(1),
    view: z.custom<PluginView>((value) => valid(value, schema.$defs.PluginView as Schema)),
  }),
});

export type PluginViewReply =
  | { readonly kind: "result"; readonly value: unknown }
  | { readonly kind: "reveal"; readonly value: z.infer<typeof revealReplySchema> }
  | { readonly kind: "saveFile"; readonly value: z.infer<typeof saveFileReplySchema> }
  | { readonly kind: "openView"; readonly value: z.infer<typeof openViewReplySchema> };

export function parsePluginViewReply(method: string, value: unknown): PluginViewReply {
  switch (method) {
    case "system.reveal_artifact":
      return { kind: "reveal", value: revealReplySchema.parse(value) };
    case "system.save_file":
      return { kind: "saveFile", value: saveFileReplySchema.parse(value) };
    case "views.open":
      return { kind: "openView", value: openViewReplySchema.parse(value) };
    default:
      // Plugin data and persisted view state cannot authorize a host UI action.
      return { kind: "result", value };
  }
}
