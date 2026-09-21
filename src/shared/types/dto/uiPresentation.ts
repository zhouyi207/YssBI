import { isRecord } from "@/shared/types/report/guards";
import { isUuid } from "@/shared/types/domain/editorProjectionGuards";
import { isResultReference, type ResultReference } from "@/shared/types/domain/result";
import {
  parseUiSpec,
  parseUiIntent,
  validateUiElement,
  type UiEvent,
  type UiIntentReceipt,
  type UiPage,
  type UiUpdate,
} from "@/shared/types/domain/uiPresentation";

const fail = (): never => {
  throw new Error("ui_spec_invalid");
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
function id(value: unknown): asserts value is string {
  if (
    typeof value !== "string" ||
    !/^[A-Za-z0-9_-]{1,64}$/.test(value) ||
    ["__proto__", "prototype", "constructor"].includes(value)
  )
    fail();
}
function source(value: unknown): asserts value is ResultReference {
  record(value, ["executionSessionId", "resultId"]);
  if (
    !isResultReference(value) ||
    value.resultId.length > 20 ||
    BigInt(value.resultId) > 18446744073709551615n
  )
    fail();
}
function revision(value: unknown): asserts value is number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) fail();
}
export function parseUiPage(value: unknown): UiPage {
  const page = record(value, ["source", "revision", "spec"]);
  source(page.source);
  revision(page.revision);
  parseUiSpec(page.spec);
  return value as UiPage;
}
export function parseUiUpdate(value: unknown): UiUpdate {
  if (!isRecord(value)) return fail();
  if (value.kind === "snapshot") {
    record(value, ["kind", "page"]);
    parseUiPage(value.page);
  } else if (value.kind === "patch") {
    record(value, ["kind", "source", "baseRevision", "revision", "operations"]);
    source(value.source);
    revision(value.baseRevision);
    revision(value.revision);
    if (
      !Array.isArray(value.operations) ||
      value.operations.length > 257 ||
      value.revision !== value.baseRevision + (value.operations.length ? 1 : 0)
    )
      return fail();
    for (const operation of value.operations) {
      if (!isRecord(operation)) return fail();
      record(operation, operation.op === "set" ? ["op", "id", "element"] : ["op", "id"]);
      id(operation.id);
      if (operation.op === "set") validateUiElement(operation.element);
      else if (operation.op !== "remove" && operation.op !== "root") fail();
    }
  } else return fail();
  return value as UiUpdate;
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
    case "update":
      record(value, ["kind", "update"]);
      parseUiUpdate(value.update);
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
