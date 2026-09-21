import { describe, expect, it } from "vitest";
import { Model } from "flexlayout-react";
import { configureWorkbenchModel } from "./workbenchActivityGroup";
import { createEmptyWorkbenchLayout } from "./workbenchLayoutDefaults";
import { WorkbenchModelOperations } from "./workbenchLayoutOperations";
import { LayoutModelBinding } from "./layoutModelBinding";
import { DEFAULT_LOGS_LAYOUT } from "./logsLayoutModel";
import { createWorkbenchLayoutController } from "../application/workbenchLayoutController";

describe("native central activation", () => {
  it("keeps the restored right Assistant selection and collapse state while installing Details", async () => {
    for (const collapsed of [false, true]) {
      const model = Model.fromJson(createEmptyWorkbenchLayout());
      const ops = new WorkbenchModelOperations(model);
      const settings = ops.ensureView({ viewId: "assistant", title: "Assistant" });
      ops.move({ panelInstanceId: settings.panelInstanceId, groupId: "border_right" });
      ops.setEdgeSize("right", 480);
      ops.setEdgeCollapsed("right", collapsed);
      const controller = createWorkbenchLayoutController({
        read: () =>
          JSON.stringify({ root: ops.serialize(), nested: { logs: DEFAULT_LOGS_LAYOUT } }),
      });
      const binding = new LayoutModelBinding(createEmptyWorkbenchLayout(), configureWorkbenchModel);
      try {
        controller.bind(binding, "main");
        await controller.whenHydrated();
        const restored = new WorkbenchModelOperations(binding.getModel());
        expect(restored.getPanel(settings.panelInstanceId)).toMatchObject({
          visible: !collapsed,
          location: { type: "edge", position: "right" },
        });
        expect(restored.getEdgeState("right").collapsed).toBe(collapsed);
        expect(
          restored.serialize().borders?.find((border) => border.location === "right")?.size,
        ).toBe(480);
        expect(
          restored
            .listPanels()
            .some((panel) => panel.metadata.role === "view" && panel.metadata.viewId === "details"),
        ).toBe(true);
      } finally {
        controller.unbind(binding);
      }
    }
  });

  it("keeps a central selection through sidebar reveals, splits and removal of the active group", () => {
    const model = Model.fromJson(createEmptyWorkbenchLayout());
    configureWorkbenchModel(model);
    const ops = new WorkbenchModelOperations(model);
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
    ops.split({
      panelInstanceId: second.panelInstanceId,
      referenceGroupId: first.groupId,
      direction: "right",
    });
    const details = ops.ensureView({ viewId: "details", title: "Details" });
    const assistant = ops.ensureView({ viewId: "assistant", title: "Assistant" });
    expect(ops.getActivePanel()?.panelInstanceId).toBe(second.panelInstanceId);
    expect(ops.getPanel(first.panelInstanceId)?.visible).toBe(true);
    expect(ops.getPanel(second.panelInstanceId)?.visible).toBe(true);
    ops.reveal(details.panelInstanceId);
    expect(ops.getActivePanel()?.panelInstanceId).toBe(second.panelInstanceId);
    ops.move({ panelInstanceId: assistant.panelInstanceId, groupId: first.groupId });
    expect(ops.getActivePanel()?.metadata.role).toBe("view");
    ops.removePanels([assistant.panelInstanceId, first.panelInstanceId]);
    expect(ops.getActivePanel()?.panelInstanceId).toBe(second.panelInstanceId);
    ops.removePanels([second.panelInstanceId]);
    expect(ops.getActivePanel()).toBeUndefined();
  });
});
