import { describe, expect, it } from "vitest";
import { Model } from "flexlayout-react";
import { configureWorkbenchModel } from "./workbenchActivityGroup";
import { createEmptyWorkbenchLayout } from "./workbenchLayoutDefaults";
import { WorkbenchModelOperations } from "./workbenchLayoutOperations";

describe("native central activation", () => {
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
