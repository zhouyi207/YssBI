import { Actions, Model, Rect, TabNode } from "flexlayout-react";
import { describe, expect, it } from "vitest";
import { createWorkbenchLayoutController } from "../application/workbenchLayoutController";
import { LayoutModelBinding } from "./layoutModelBinding";
import { DEFAULT_LOGS_LAYOUT } from "./logsLayoutModel";
import { configureWorkbenchModel } from "./workbenchActivityGroup";
import { createEmptyWorkbenchLayout } from "./workbenchLayoutDefaults";
import { WorkbenchModelOperations } from "./workbenchLayoutOperations";
import { PendingWorkbenchTransaction } from "./workbenchLayoutTransaction";
import { createPersistedWorkbenchLayout, isValidRootLayout } from "./workbenchLayoutPersistence";

function floatSettings(model: Model, id: string): string {
  const layoutId = model.getNodeById(id)!.getLayoutId();
  model.doAction(Actions.moveFloat(layoutId, new Rect(20, 30, 900, 600)));
  return layoutId;
}

describe("workbench floating Settings", () => {
  it("notifies the application once per float gesture while invalidating stale transactions on every move", () => {
    const binding = new LayoutModelBinding(createEmptyWorkbenchLayout(), configureWorkbenchModel);
    const model = binding.getModel();
    const ops = new WorkbenchModelOperations(model);
    const settings = ops.ensureView({ viewId: "settings", title: "Settings" });
    const layoutId = floatSettings(model, settings.panelInstanceId);
    const pending = new PendingWorkbenchTransaction(binding);
    const revision = binding.getSnapshot().revision;
    let notifications = 0;
    let nativeMoves = 0;
    const unsubscribe = binding.subscribe(() => notifications++);
    model.addChangeListener({
      onAfterAction: (action) => {
        if (action.type === Actions.MOVE_FLOAT) nativeMoves++;
      },
    });
    for (let index = 1; index <= 100; index++) {
      model.doAction(Actions.moveFloat(layoutId, new Rect(index, 30, 900, 600)).setAdjusting(true));
    }
    expect(nativeMoves).toBe(100);
    expect(ops.serialize().subLayouts?.[layoutId]?.rect?.x).toBe(100);
    expect(binding.getSnapshot().revision).toBe(revision + 100);
    expect(() => pending.commit()).toThrow("layout_restore_failed");
    expect(notifications).toBe(0);
    model.doAction(Actions.moveFloat(layoutId, new Rect(100, 30, 900, 600)).setAdjusting(false));
    expect(nativeMoves).toBe(101);
    expect(notifications).toBe(1);
    unsubscribe();
  });

  it("opens and restores Settings only as a singleton float without changing the active editor", async () => {
    const model = Model.fromJson(createEmptyWorkbenchLayout());
    configureWorkbenchModel(model);
    const ops = new WorkbenchModelOperations(model);
    const editor = ops.openEditor({
      resourceKind: "event",
      resourceRef: "events/Main",
      title: "Main",
      mode: "reuse-resource",
    });
    const request = { viewId: "settings", title: "Settings" } as const;
    const settings = ops.ensureView(request);
    expect((model.getNodeById(settings.panelInstanceId) as TabNode).isEnableFloat()).toBe(false);
    expect((model.getNodeById(settings.panelInstanceId) as TabNode).isEnableFloatIcon()).toBe(
      false,
    );
    const layoutId = floatSettings(model, settings.panelInstanceId);
    model.doAction(Actions.maximizeToggle(editor.groupId));
    expect(ops.getPanel(settings.panelInstanceId)?.visible).toBe(true);
    const persisted = createPersistedWorkbenchLayout(ops.serialize(), DEFAULT_LOGS_LAYOUT);
    expect(isValidRootLayout(persisted.root)).toBe(true);
    const controller = createWorkbenchLayoutController({ read: () => JSON.stringify(persisted) });
    const binding = new LayoutModelBinding(createEmptyWorkbenchLayout(), configureWorkbenchModel);
    try {
      controller.bind(binding, "main");
      await controller.whenHydrated();
      const restored = new WorkbenchModelOperations(binding.getModel());
      expect(restored.ensureView(request)).toMatchObject({
        panelInstanceId: settings.panelInstanceId,
        location: { type: "float", layoutId },
        visible: true,
      });
      expect(restored.serialize().subLayouts?.[layoutId]?.rect).toEqual({
        x: 20,
        y: 30,
        width: 900,
        height: 600,
      });
      expect(restored.getActivePanel()?.panelInstanceId).toBe(editor.panelInstanceId);
      expect(
        restored.move({
          panelInstanceId: settings.panelInstanceId,
          groupId: restored.ensureCentralGroup(),
        }),
      ).toBe(false);
      expect(restored.getPanel(settings.panelInstanceId)?.location.type).toBe("float");
      restored.removePanels([settings.panelInstanceId]);
      expect(restored.getPanel(settings.panelInstanceId)).toBeUndefined();
      expect(Object.keys(restored.serialize().subLayouts ?? {})).toEqual([]);
    } finally {
      controller.unbind(binding);
    }
  });

  it("rejects unsupported float contents and browser popouts", () => {
    const model = Model.fromJson(createEmptyWorkbenchLayout());
    configureWorkbenchModel(model);
    const ops = new WorkbenchModelOperations(model);
    const settings = ops.ensureView({ viewId: "settings", title: "Settings" });
    const layoutId = floatSettings(model, settings.panelInstanceId);
    const floatingGroup = ops.getPanel(settings.panelInstanceId)!.groupId;
    const editor = ops.openEditor({
      resourceKind: "event",
      resourceRef: "events/Main",
      title: "Main",
      mode: "reuse-resource",
    });
    expect(ops.move({ panelInstanceId: editor.panelInstanceId, groupId: floatingGroup })).toBe(
      false,
    );
    expect(
      ops.split({
        panelInstanceId: editor.panelInstanceId,
        referenceGroupId: floatingGroup,
        direction: "right",
      }),
    ).toBe(false);
    const browser = ops.serialize();
    browser.subLayouts![layoutId].type = "window";
    expect(isValidRootLayout(browser)).toBe(false);
    model.doAction(
      Actions.updateNodeAttributes(settings.panelInstanceId, {
        component: "Details",
        config: { metadata: { role: "view", viewId: "details" } },
      }),
    );
    expect(isValidRootLayout(ops.serialize())).toBe(false);
  });
});
