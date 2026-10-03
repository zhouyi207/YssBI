import { Actions, Model, TabNode, type IJsonTabNode, type IJsonTabSetNode } from "flexlayout-react";
import { expect, it } from "vitest";
import { logDomainPanelId } from "@/features/domain/log/logDomains";
import { LayoutModelBinding } from "./layoutModelBinding";
import { createDefaultLogsLayout } from "./logsLayoutModel";
import { createLogsLayoutRuntime, type LogsLayoutRuntime } from "./logsRuntime";

const tabId = logDomainPanelId("all");

function storedTab(runtime: LogsLayoutRuntime): TabNode {
  const tab = Model.fromJson(runtime.getLatestSnapshot()).getNodeById(tabId);
  if (!(tab instanceof TabNode)) throw new Error("Expected Logs tab");
  return tab;
}

it("isolates restored and exported layouts while capturing native save events on every request", () => {
  const defaults = createDefaultLogsLayout();
  const runtime = createLogsLayoutRuntime(defaults);
  defaults.global!.tabEnableClose = true;
  expect(runtime.getLatestSnapshot().global!.tabEnableClose).toBe(false);

  const restored = createDefaultLogsLayout();
  const group = restored.layout.children![0] as IJsonTabSetNode;
  const tab = group.children![0] as IJsonTabNode;
  tab.config = { ...tab.config, state: { captured: 0 } };
  expect(runtime.stageRestore(runtime.beginRestore(), restored)).toBe("staged");
  tab.config.state.captured = 90;
  expect(storedTab(runtime).getConfig().state.captured).toBe(0);

  const binding = new LayoutModelBinding(createDefaultLogsLayout());
  runtime.bind(binding);
  try {
    const node = binding.getModel().getNodeById(tabId);
    if (!(node instanceof TabNode)) throw new Error("Expected Logs tab");
    const published = runtime.getLatestSnapshot();
    published.global!.tabEnableClose = true;
    expect(runtime.getLatestSnapshot().global!.tabEnableClose).toBe(false);

    let saves = 0;
    node.setEventListener("save", () => {
      node.getConfig().state.captured = ++saves;
    });
    runtime.captureBoundSnapshot();
    expect(storedTab(runtime).getConfig().state.captured).toBe(1);
    node.getConfig().state.captured = 99;
    expect(storedTab(runtime).getConfig().state.captured).toBe(1);
    runtime.captureBoundSnapshot();
    expect(storedTab(runtime).getConfig().state.captured).toBe(2);
    expect(tab.config.state.captured).toBe(90);
  } finally {
    runtime.unbind(binding);
  }
});

it("retains a replacement binding installed by an observer of the old binding's final capture", () => {
  const runtime = createLogsLayoutRuntime();
  const previous = new LayoutModelBinding(createDefaultLogsLayout());
  const replacement = new LayoutModelBinding(createDefaultLogsLayout());
  runtime.bind(previous);
  let requested: LayoutModelBinding | undefined = replacement;
  const stop = runtime.subscribe(() => {
    const target = requested;
    if (!target) return;
    requested = undefined;
    runtime.bind(target);
  });
  try {
    previous
      .getModel()
      .doAction(
        Actions.updateNodeAttributes(tabId, { name: "Final old state" }).setAdjusting(true),
      );
    runtime.unbind(previous);
    expect(requested).toBeUndefined();
    expect(storedTab(runtime).getName()).toBe("Final old state");
    replacement
      .getModel()
      .doAction(Actions.updateNodeAttributes(tabId, { name: "New live state" }));
    expect(storedTab(runtime).getName()).toBe("New live state");

    const outer = new LayoutModelBinding(createDefaultLogsLayout());
    const latest = new LayoutModelBinding(createDefaultLogsLayout());
    requested = latest;
    replacement
      .getModel()
      .doAction(
        Actions.updateNodeAttributes(tabId, { name: "Pending handoff" }).setAdjusting(true),
      );
    runtime.bind(outer);
    expect(requested).toBeUndefined();
    latest.getModel().doAction(Actions.updateNodeAttributes(tabId, { name: "Latest binding" }));
    expect(storedTab(runtime).getName()).toBe("Latest binding");
    outer.getModel().doAction(Actions.updateNodeAttributes(tabId, { name: "Obsolete outer bind" }));
    expect(storedTab(runtime).getName()).toBe("Latest binding");

    runtime.unbind(latest);
    let restoreBinding: (() => void) | undefined;
    const sameApi = new LayoutModelBinding(createDefaultLogsLayout(), () => restoreBinding?.());
    restoreBinding = () => {
      restoreBinding = undefined;
      runtime.unbind(sameApi);
      runtime.bind(sameApi);
      throw new Error("Original restore failed");
    };
    expect(() => runtime.bind(sameApi)).toThrow("Original restore failed");
    sameApi
      .getModel()
      .doAction(Actions.updateNodeAttributes(tabId, { name: "Same API successor" }));
    expect(storedTab(runtime).getName()).toBe("Same API successor");
  } finally {
    stop();
    runtime.unbind();
  }
});
