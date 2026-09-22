import { Actions, Model, Rect } from "flexlayout-react";
import { describe, expect, it } from "vitest";
import {
  createWorkbenchLayoutController,
  workbenchLayoutController,
} from "../application/workbenchLayoutController";
import { resetWorkbenchLayout } from "../application/workbenchLayoutActions";
import { LayoutModelBinding } from "./layoutModelBinding";
import { DEFAULT_LOGS_LAYOUT } from "./logsLayoutModel";
import { configureWorkbenchModel } from "./workbenchActivityGroup";
import { createEmptyWorkbenchLayout } from "./workbenchLayoutDefaults";
import { WorkbenchModelOperations } from "./workbenchLayoutOperations";
import { PendingWorkbenchTransaction } from "./workbenchLayoutTransaction";
import { createPersistedWorkbenchLayout, isValidRootLayout } from "./workbenchLayoutPersistence";

function floatEditor(model: Model, id: string): string {
  new WorkbenchModelOperations(model).floatPanel(id);
  const layoutId = model.getNodeById(id)!.getLayoutId();
  model.doAction(Actions.moveFloat(layoutId, new Rect(20, 30, 900, 600)));
  return layoutId;
}

describe("workbench floating panels", () => {
  it("floats an entire group and resets its tabs to the main workspace with their identities and selection intact", async () => {
    const binding = new LayoutModelBinding(createEmptyWorkbenchLayout(), configureWorkbenchModel);
    try {
      workbenchLayoutController.bind(binding, "float-reset");
      await workbenchLayoutController.whenHydrated();
      const ops = new WorkbenchModelOperations(binding.getModel());
      const first = ops.openEditor({
        resourceKind: "event",
        resourceRef: "events/A",
        title: "A",
        mode: "reuse-resource",
      });
      const second = ops.openEditor({
        resourceKind: "event",
        resourceRef: "events/B",
        title: "B",
        mode: "reuse-resource",
      });
      expect(ops.floatGroup(first.groupId)).toBe(true);
      const layoutId = binding.getModel().getNodeById(first.panelInstanceId)!.getLayoutId();
      binding.getModel().doAction(Actions.moveFloat(layoutId, new Rect(20, 30, 900, 600)));
      expect(ops.listGroupPanels(first.groupId).map((panel) => panel.panelInstanceId)).toEqual([
        first.panelInstanceId,
        second.panelInstanceId,
      ]);
      expect(ops.getActivePanel()?.panelInstanceId).toBe(second.panelInstanceId);
      await resetWorkbenchLayout();
      const reset = new WorkbenchModelOperations(binding.getModel());
      expect(reset.getPanel(first.panelInstanceId)?.location.type).toBe("grid");
      expect(reset.getPanel(second.panelInstanceId)?.groupId).toBe(
        reset.getPanel(first.panelInstanceId)?.groupId,
      );
      expect(reset.getActivePanel()?.panelInstanceId).toBe(second.panelInstanceId);
      expect(Object.keys(reset.serialize().subLayouts ?? {})).toEqual([]);
    } finally {
      workbenchLayoutController.unbind(binding);
    }
  });
  it("notifies the application once per float gesture while invalidating stale transactions on every move", () => {
    const binding = new LayoutModelBinding(createEmptyWorkbenchLayout(), configureWorkbenchModel);
    const model = binding.getModel();
    const ops = new WorkbenchModelOperations(model);
    const floating = ops.openEditor({
      resourceKind: "event",
      resourceRef: "events/Float",
      title: "Float",
      mode: "reuse-resource",
    });
    const layoutId = floatEditor(model, floating.panelInstanceId);
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

  it("restores a floating editor as the command target and docks it without changing identity", async () => {
    const model = Model.fromJson(createEmptyWorkbenchLayout());
    configureWorkbenchModel(model);
    const ops = new WorkbenchModelOperations(model);
    const first = ops.openEditor({
      resourceKind: "event",
      resourceRef: "events/Main",
      title: "Main",
      mode: "reuse-resource",
    });
    const editor = ops.openEditor({
      resourceKind: "event",
      resourceRef: "events/Float",
      title: "Float",
      mode: "reuse-resource",
    });
    const layoutId = floatEditor(model, editor.panelInstanceId);
    expect(ops.getActivePanel()?.panelInstanceId).toBe(editor.panelInstanceId);
    ops.activate(first.panelInstanceId);
    expect(ops.getActivePanel()?.panelInstanceId).toBe(first.panelInstanceId);
    ops.activate(editor.panelInstanceId);
    model.doAction(Actions.maximizeToggle(first.groupId));
    expect(ops.getPanel(editor.panelInstanceId)?.visible).toBe(true);
    const persisted = createPersistedWorkbenchLayout(ops.serialize(), DEFAULT_LOGS_LAYOUT);
    expect(isValidRootLayout(persisted.root)).toBe(true);
    const controller = createWorkbenchLayoutController({ read: () => JSON.stringify(persisted) });
    const binding = new LayoutModelBinding(createEmptyWorkbenchLayout(), configureWorkbenchModel);
    try {
      controller.bind(binding, "main");
      await controller.whenHydrated();
      const restored = new WorkbenchModelOperations(binding.getModel());
      expect(restored.getPanel(editor.panelInstanceId)).toMatchObject({
        location: { type: "float", layoutId },
        visible: true,
      });
      expect(restored.getActivePanel()?.panelInstanceId).toBe(editor.panelInstanceId);
      expect(restored.serialize().subLayouts?.[layoutId]?.rect).toEqual({
        x: 20,
        y: 30,
        width: 900,
        height: 600,
      });
      expect(restored.dockFloat(layoutId)).toBe(true);
      expect(restored.getPanel(editor.panelInstanceId)?.location.type).toBe("grid");
      expect(restored.getPanel(editor.panelInstanceId)?.visible).toBe(true);
      expect(restored.getActivePanel()?.panelInstanceId).toBe(editor.panelInstanceId);
      expect(Object.keys(restored.serialize().subLayouts ?? {})).toEqual([]);
    } finally {
      controller.unbind(binding);
    }
  });

  it("rejects unsupported float contents and browser popouts", () => {
    const model = Model.fromJson(createEmptyWorkbenchLayout());
    configureWorkbenchModel(model);
    const ops = new WorkbenchModelOperations(model);
    const floating = ops.openEditor({
      resourceKind: "event",
      resourceRef: "events/Float",
      title: "Float",
      mode: "reuse-resource",
    });
    const layoutId = floatEditor(model, floating.panelInstanceId);
    const floatingGroup = ops.getPanel(floating.panelInstanceId)!.groupId;
    const editor = ops.openEditor({
      resourceKind: "event",
      resourceRef: "events/Main",
      title: "Main",
      mode: "reuse-resource",
    });
    expect(ops.move({ panelInstanceId: editor.panelInstanceId, groupId: floatingGroup })).toBe(
      true,
    );
    expect(
      ops.split({
        panelInstanceId: editor.panelInstanceId,
        referenceGroupId: floatingGroup,
        direction: "right",
      }),
    ).toBe(true);
    const browser = ops.serialize();
    expect(isValidRootLayout({ ...browser, popouts: {} })).toBe(false);
    browser.subLayouts![layoutId].type = "window";
    expect(isValidRootLayout(browser)).toBe(false);
    model.doAction(
      Actions.updateNodeAttributes(floating.panelInstanceId, {
        component: "Details",
        config: { metadata: { role: "view", viewId: "details" } },
      }),
    );
    expect(isValidRootLayout(ops.serialize())).toBe(false);
  });
});
