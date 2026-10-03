import { isRecord } from "@/shared/types/report/guards";
import { isUuid } from "@/shared/types/domain/editorProjectionGuards";
import {
  parseUiIntent,
  type UiEvent,
  type UiIntentReceipt,
} from "@/shared/types/domain/uiPresentation";

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
export function parseUiIntentReceipt(value: unknown): UiIntentReceipt {
  const receipt = record(value, ["id", "intent", "status"]);
  if (
    !isUuid(receipt.id) ||
    !["pending", "claimed", "applied", "failed", "expired"].includes(receipt.status as string)
  )
    fail();
  parseUiIntent(receipt.intent);
  return value as UiIntentReceipt;
}
export function parseUiEvent(value: unknown): UiEvent {
  if (!isRecord(value)) return fail();
  switch (value.kind) {
    case "resync":
    case "sessionChanged":
      record(value, ["kind"]);
      break;
    case "intent":
      record(value, ["kind", "receipt"]);
      parseUiIntentReceipt(value.receipt);
      break;
    default:
      return fail();
  }
  return value as UiEvent;
}
