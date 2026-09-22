import { isResultReference, resultReferenceKey, type ResultReference } from "./result";
import { isRecord } from "@/shared/types/report/guards";
import { isUuid } from "./editorProjectionGuards";

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
  | { readonly kind: "openGraph"; readonly graphPath: string; readonly nodeId: string | null }
  | { readonly kind: "openResult"; readonly source: ResultReference }
  | { readonly kind: "showPanel"; readonly panel: (typeof UI_PANELS)[number] };
export type UiComponent =
  | { readonly type: "column" | "row"; readonly props: { readonly gap: number } }
  | { readonly type: "text"; readonly props: { readonly text: string } }
  | {
      readonly type: "section";
      readonly props: { readonly title: string; readonly collapsible: boolean };
    }
  | UiBoundComponent
  | {
      readonly type: "button";
      readonly props: { readonly label: string; readonly intent: UiIntent };
    };
export type UiBindingKind =
  | "equation"
  | "keyValue"
  | "table"
  | "statCard"
  | "coefficientTable"
  | "chart"
  | "analysis";
export interface UiBoundComponent {
  readonly type: UiBindingKind;
  readonly props: { readonly binding: string };
}
export interface UiElement {
  readonly component: UiComponent;
  readonly visible: boolean;
  readonly children: readonly string[];
}
export interface UiSpec {
  readonly root: string;
  readonly elements: Readonly<Record<string, UiElement>>;
}
export interface UiPage {
  readonly source: ResultReference;
  readonly revision: number;
  readonly spec: UiSpec;
}
export type UiPatch =
  | { readonly op: "set"; readonly id: string; readonly element: UiElement }
  | { readonly op: "remove" | "root"; readonly id: string };
export type UiUpdate =
  | { readonly kind: "snapshot"; readonly page: UiPage }
  | {
      readonly kind: "patch";
      readonly source: ResultReference;
      readonly baseRevision: number;
      readonly revision: number;
      readonly operations: readonly UiPatch[];
    };
export type UiAction =
  | { readonly kind: "replace"; readonly spec: UiSpec }
  | { readonly kind: "patch"; readonly operations: readonly UiPatch[] }
  | { readonly kind: "visibility"; readonly id: string; readonly visible: boolean }
  | { readonly kind: "move"; readonly id: string; readonly offset: -1 | 1 }
  | { readonly kind: "reset" };
export type UiIntentStatus = "pending" | "claimed" | "applied" | "failed" | "expired";
export interface UiIntentReceipt {
  readonly id: string;
  readonly intent: UiIntent;
  readonly status: UiIntentStatus;
}
export type UiEvent =
  | { readonly kind: "update"; readonly update: UiUpdate }
  | { readonly kind: "intent"; readonly receipt: UiIntentReceipt }
  | { readonly kind: "resync" | "sessionChanged" };

export const UI_SPEC_BYTE_LIMIT = 65_536;
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
function text(value: unknown, maximum: number): asserts value is string {
  if (typeof value !== "string" || new TextEncoder().encode(value).length > maximum) fail();
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
export function parseUiIntent(value: unknown): UiIntent {
  if (!isRecord(value)) return fail();
  switch (value.kind) {
    case "openGraph":
      record(value, ["kind", "graphPath", "nodeId"]);
      text(value.graphPath, 4096);
      if (!value.graphPath || (value.nodeId !== null && !isUuid(value.nodeId))) fail();
      break;
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
export function validateUiElement(value: unknown): asserts value is UiElement {
  const item = record(value, ["component", "visible", "children"]);
  if (
    typeof item.visible !== "boolean" ||
    !Array.isArray(item.children) ||
    item.children.length > 128
  )
    fail();
  (item.children as unknown[]).forEach(id);
  const component = record(item.component, ["type", "props"]);
  switch (component.type) {
    case "column":
    case "row": {
      const props = record(component.props, ["gap"]);
      if (
        typeof props.gap !== "number" ||
        !Number.isInteger(props.gap) ||
        props.gap < 0 ||
        props.gap > 8
      )
        fail();
      return;
    }
    case "text":
      text(record(component.props, ["text"]).text, 8192);
      break;
    case "section": {
      const props = record(component.props, ["title", "collapsible"]);
      text(props.title, 256);
      if (!props.title.trim() || typeof props.collapsible !== "boolean") fail();
      return;
    }
    case "equation":
    case "keyValue":
    case "table":
    case "statCard":
    case "coefficientTable":
    case "chart":
    case "analysis":
      id(record(component.props, ["binding"]).binding);
      break;
    case "button": {
      const props = record(component.props, ["label", "intent"]);
      text(props.label, 256);
      if (!props.label.trim()) fail();
      parseUiIntent(props.intent);
      break;
    }
    default:
      fail();
  }
  if ((item.children as unknown[]).length !== 0) fail();
}
export function parseUiSpec(value: unknown): UiSpec {
  const spec = record(value, ["root", "elements"]);
  id(spec.root);
  if (
    !isRecord(spec.elements) ||
    Object.keys(spec.elements).length > 128 ||
    new TextEncoder().encode(JSON.stringify(value)).length > UI_SPEC_BYTE_LIMIT
  )
    return fail();
  for (const [key, item] of Object.entries(spec.elements)) {
    id(key);
    validateUiElement(item);
  }
  const elements = spec.elements as Record<string, UiElement>;
  const visited = new Set<string>();
  const visit = (key: string, depth: number) => {
    if (depth > 12 || visited.has(key) || !hasOwn(elements, key)) fail();
    visited.add(key);
    elements[key].children.forEach((child) => visit(child, depth + 1));
  };
  visit(spec.root, 0);
  if (visited.size !== Object.keys(elements).length) fail();
  return value as UiSpec;
}
export function installUiUpdate(
  previous: UiPage | null,
  update: UiUpdate,
  expected: ResultReference,
): UiPage {
  const incoming = update.kind === "snapshot" ? update.page.source : update.source;
  if (resultReferenceKey(incoming) !== resultReferenceKey(expected)) return fail();
  const nextRevision = update.kind === "snapshot" ? update.page.revision : update.revision;
  revision(nextRevision);
  if (previous && nextRevision <= previous.revision) return previous;
  if (update.kind === "snapshot") return update.page;
  if (!previous || previous.revision !== update.baseRevision) return fail();
  const elements = { ...previous.spec.elements };
  let root = previous.spec.root;
  for (const operation of update.operations) {
    if (operation.op === "set") elements[operation.id] = operation.element;
    else if (operation.op === "root") root = operation.id;
    else {
      if (!hasOwn(elements, operation.id)) return fail();
      delete elements[operation.id];
    }
  }
  const spec = parseUiSpec({ root, elements });
  return { source: previous.source, revision: update.revision, spec };
}
