import { Actions, Model, TabNode, type IJsonModel } from "flexlayout-react";
import { isLayoutJson, isRecord } from "./layoutSerialization";
import { isValidLogsLayout } from "./logsLayoutModel";
import { createEmptyWorkbenchLayout } from "./workbenchLayoutDefaults";
import { canMoveWorkbenchPanel } from "./workbenchActivityGroup";
import {
  componentForWorkbenchMetadata,
  isWorkbenchPanelMetadata,
  type WorkbenchPanelMetadata,
} from "./workbenchPanelModel";

export interface PersistedWorkbenchLayout {
  readonly root: IJsonModel;
  readonly nested: { readonly logs: IJsonModel };
}
export type ParsedLayoutPart<T> =
  | { readonly status: "valid"; readonly value: T }
  | { readonly status: "invalid" };
export interface ParsedPersistedWorkbenchLayout {
  readonly root: ParsedLayoutPart<IJsonModel>;
  readonly logs: ParsedLayoutPart<IJsonModel>;
}

export function isValidRootLayout(candidate: unknown): candidate is IJsonModel {
  const identities = new Set<string>();
  return isLayoutJson(candidate, (tab, parent) => {
    if (!isRecord(tab.config) || !isWorkbenchPanelMetadata(tab.config.metadata)) return false;
    const metadata = tab.config.metadata;
    if (
      componentForWorkbenchMetadata(metadata) !== tab.component ||
      !canMoveWorkbenchPanel(metadata, parent)
    )
      return false;
    const key =
      metadata.role === "view"
        ? "view:" + metadata.viewId
        : metadata.role === "plugin"
          ? "plugin:" + metadata.pluginId + ":" + metadata.viewId
          : metadata.role === "result"
            ? "result:" + metadata.reference.executionSessionId + ":" + metadata.reference.resultId
            : undefined;
    if (key && identities.has(key)) return false;
    if (key) identities.add(key);
    return true;
  });
}
function normalize(layout: IJsonModel): IJsonModel {
  const copy = structuredClone(layout);
  copy.global = { ...copy.global, ...createEmptyWorkbenchLayout().global };
  const borders = copy.borders ?? [];
  for (const border of createEmptyWorkbenchLayout().borders ?? []) {
    if (!borders.some((existing) => existing.location === border.location)) borders.push(border);
  }
  copy.borders = borders;
  return copy;
}
function withoutPanels(
  layout: IJsonModel,
  remove: (metadata: WorkbenchPanelMetadata) => boolean,
): IJsonModel {
  const model = Model.fromJson(normalize(layout));
  const ids: string[] = [];
  model.visitNodes((node) => {
    if (!(node instanceof TabNode)) return;
    const metadata: unknown = node.getConfig()?.metadata;
    if (isWorkbenchPanelMetadata(metadata) && remove(metadata)) ids.push(node.getId());
  });
  if (ids.length) model.doAction(Actions.group(ids.map(Actions.deleteTab)));
  return model.toJson();
}
export function workbenchLayoutStorageKey(label: string): string {
  return "yssbi-workbench-flexlayout:" + (label || "main");
}
export function createPersistedWorkbenchLayout(
  root: IJsonModel,
  logs: IJsonModel,
): PersistedWorkbenchLayout {
  return { root: prepareRootLayoutForPersistence(root), nested: { logs: structuredClone(logs) } };
}
export function parsePersistedWorkbenchLayout(
  candidate: unknown,
): ParsedPersistedWorkbenchLayout | null {
  if (!isRecord(candidate) || !isRecord(candidate.nested)) return null;
  return {
    root: isValidRootLayout(candidate.root)
      ? { status: "valid", value: normalize(candidate.root) }
      : { status: "invalid" },
    logs: isValidLogsLayout(candidate.nested.logs)
      ? { status: "valid", value: candidate.nested.logs }
      : { status: "invalid" },
  };
}
export function prepareRootLayoutForPersistence(layout: IJsonModel): IJsonModel {
  return withoutPanels(
    layout,
    (metadata) =>
      metadata.role === "result" || (metadata.role === "view" && metadata.viewId === "inspect"),
  );
}
export function scrubProjectScopedRootLayout(layout: IJsonModel): IJsonModel {
  return withoutPanels(
    layout,
    (metadata) =>
      metadata.role === "editor" ||
      metadata.role === "result" ||
      (metadata.role === "view" && metadata.viewId === "inspect"),
  );
}
