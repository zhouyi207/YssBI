import { isResultReference, type ResultReference } from "./result";
import { isRecord } from "@/shared/types/report/guards";
import { isUuid } from "./editorProjectionGuards";
import { RESOURCE_KINDS, type ResourceRef } from "./resource";

export const UI_PANELS = [
  "project",
  "nodes",
  "commands",
  "details",
  "assistant",
  "problems",
  "output",
  "logs",
] as const;
export type UiIntent =
  | {
      readonly kind: "openResource";
      readonly resource: Readonly<ResourceRef>;
      readonly nodeId: string | null;
    }
  | { readonly kind: "openResult"; readonly source: ResultReference }
  | { readonly kind: "showPanel"; readonly panel: (typeof UI_PANELS)[number] };
export type UiIntentStatus = "pending" | "claimed" | "applied" | "failed" | "expired";
export interface UiIntentReceipt {
  readonly id: string;
  readonly intent: UiIntent;
  readonly status: UiIntentStatus;
}
export type UiEvent =
  | { readonly kind: "intent"; readonly receipt: UiIntentReceipt }
  | { readonly kind: "resync" | "sessionChanged" };

const fail = (): never => {
  throw new Error("ui_intent_invalid");
};
const hasOwn = (value: object, key: string) => Object.prototype.hasOwnProperty.call(value, key);
function record(value: unknown, keys: readonly string[]): Record<string, unknown> {
  if (
    !isRecord(value) ||
    Object.keys(value).length !== keys.length ||
    !keys.every((key) => hasOwn(value, key))
  )
    return fail();
  return value;
}
function text(value: unknown, maximum: number): asserts value is string {
  if (typeof value !== "string" || new TextEncoder().encode(value).length > maximum) fail();
}
function source(value: unknown): asserts value is ResultReference {
  record(value, ["executionSessionId", "resultId"]);
  if (!isResultReference(value)) fail();
}
export function parseUiIntent(value: unknown): UiIntent {
  if (!isRecord(value)) return fail();
  switch (value.kind) {
    case "openResource": {
      record(value, ["kind", "resource", "nodeId"]);
      const resource = record(value.resource, ["kind", "id"]);
      text(resource.id, 4096);
      if (
        !resource.id ||
        !RESOURCE_KINDS.includes(resource.kind as never) ||
        (value.nodeId !== null &&
          (!isUuid(value.nodeId) ||
            (resource.kind !== "event_graph" && resource.kind !== "function_graph")))
      )
        fail();
      break;
    }
    case "openResult":
      record(value, ["kind", "source"]);
      source(value.source);
      break;
    case "showPanel":
      record(value, ["kind", "panel"]);
      if (!UI_PANELS.includes(value.panel as never)) fail();
      break;
    default:
      return fail();
  }
  return value as UiIntent;
}
