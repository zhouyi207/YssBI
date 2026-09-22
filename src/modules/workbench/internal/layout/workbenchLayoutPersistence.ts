import {
  Actions,
  Model,
  TabNode,
  type IJsonModel,
  type IJsonRowNode,
  type IJsonTabNode,
  type IJsonTabSetNode,
} from "flexlayout-react";
import { isLayoutJson, isRecord } from "./layoutSerialization";
import { isValidLogsLayout } from "./logsLayoutModel";
import { createEmptyWorkbenchLayout } from "./workbenchLayoutDefaults";
import {
  canMoveWorkbenchPanel,
  hasWorkbenchPanelCloseButton,
  canFloatWorkbenchPanel,
} from "./workbenchActivityGroup";
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
  return isLayoutJson(
    candidate,
    (tab, parent, floating) => {
      if (
        !isRecord(tab.config) ||
        Object.keys(tab.config).length !== 1 ||
        !isWorkbenchPanelMetadata(tab.config.metadata)
      )
        return false;
      const metadata = tab.config.metadata;
      if (
        componentForWorkbenchMetadata(metadata) !== tab.component ||
        !canMoveWorkbenchPanel(metadata, parent, floating ? "float" : undefined)
      )
        return false;
      const key =
        metadata.role === "view"
          ? "view:" + metadata.viewId
          : metadata.role === "plugin"
            ? "plugin:" + metadata.pluginId + ":" + metadata.viewId
            : metadata.role === "result"
              ? "result:" +
                metadata.reference.executionSessionId +
                ":" +
                metadata.reference.resultId
              : undefined;
      if (key && identities.has(key)) return false;
      if (key) identities.add(key);
      return true;
    },
    true,
  );
}
function normalize(layout: IJsonModel): IJsonModel {
  const copy = structuredClone(layout);
  copy.global = { ...copy.global, ...createEmptyWorkbenchLayout().global };
  const normalizeTabs = (children: IJsonTabSetNode["children"]): void => {
    for (const child of children ?? []) {
      if (child.type !== "tab") continue;
      const tab = child as IJsonTabNode;
      const metadata: unknown = tab.config?.metadata;
      if (isWorkbenchPanelMetadata(metadata)) {
        tab.enableClose = hasWorkbenchPanelCloseButton(metadata);
        tab.enableFloat = canFloatWorkbenchPanel(metadata);
        delete tab.enableFloatIcon;
        tab.enablePopout = false;
      }
    }
  };
  const normalizeTabsets = (node: IJsonRowNode | IJsonTabSetNode): void => {
    if (node.type === "tabset") {
      // Host policy requires empty groups to be reclaimed regardless of per-node overrides.
      const tabset = node as IJsonTabSetNode;
      delete tabset.enableClose;
      delete tabset.enableDeleteWhenEmpty;
      delete tabset.enableTabStrip;
      delete tabset.enableDrag;
      delete tabset.enableDivide;
      normalizeTabs(tabset.children);
      return;
    }
    for (const child of node.children ?? []) {
      if (child.type === "row" || child.type === "tabset") normalizeTabsets(child);
    }
  };
  normalizeTabsets(copy.layout);
  for (const layout of Object.values(copy.subLayouts ?? {})) {
    normalizeTabsets(layout.layout);
  }
  const borders = copy.borders ?? [];
  for (const border of createEmptyWorkbenchLayout().borders ?? []) {
    const existing = borders.find((candidate) => candidate.location === border.location);
    if (!existing) borders.push(border);
    else if (border.location === "bottom") {
      existing.enableAutoHide = false;
      existing.show = true;
    }
  }
  for (const border of borders) normalizeTabs(border.children);
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
  return withoutPanels(layout, (metadata) => metadata.role === "result");
}
export function scrubProjectScopedRootLayout(layout: IJsonModel): IJsonModel {
  return withoutPanels(
    layout,
    (metadata) => metadata.role === "editor" || metadata.role === "result",
  );
}
