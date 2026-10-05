import { describe, expect, it } from "vitest";
import { Actions, Model } from "flexlayout-react";
import { configureWorkbenchModel } from "./workbenchActivityGroup";
import {
  createEmptyWorkbenchLayout,
  WORKBENCH_CONVERSATION_GROUP_ID,
  WORKBENCH_WORKSPACE_LAYOUT_ID,
  WORKBENCH_WORKSPACE_GROUP_ID,
} from "./workbenchLayoutDefaults";
import {
  parsePersistedWorkbenchLayout,
  scrubProjectScopedRootLayout,
} from "./workbenchLayoutPersistence";
import { WorkbenchModelOperations } from "./workbenchLayoutOperations";
import { LayoutModelBinding } from "./layoutModelBinding";
import { DEFAULT_LOGS_LAYOUT } from "./logsLayoutModel";
import { createWorkbenchLayoutController } from "../application/workbenchLayoutController";

describe("native central activation", () => {
  it("keeps conversation tabs beside the workspace until each is closed", () => {
    const model = Model.fromJson(createEmptyWorkbenchLayout());
    configureWorkbenchModel(model);
    const ops = new WorkbenchModelOperations(model);
    const first = ops.openConversation({ sessionId: "conversation-1", title: "First" });
    const editor = ops.openEditor({
      resourceKind: "event_graph",
      resourceRef: "events/A",
      title: "A",
      mode: "reuse-resource",
    });
    const second = ops.openConversation({ sessionId: "conversation-2", title: "Second" });
    expect(first.groupId).toBe(WORKBENCH_CONVERSATION_GROUP_ID);
    expect(model.getNodeById(first.groupId)?.toJson()).toMatchObject({ enableTabStrip: false });
    expect(second.groupId).toBe(first.groupId);
    expect(editor.groupId).not.toBe(first.groupId);
    expect(first.location.type).toBe("conversation");
    expect(model.getNodeById(first.groupId)?.getLayoutId()).toBe(Model.MAIN_LAYOUT_ID);
    expect(model.getNodeById(editor.groupId)?.getLayoutId()).toBe(WORKBENCH_WORKSPACE_LAYOUT_ID);
    expect(ops.serialize().layout.children?.[0]?.id).toBe(first.groupId);
    expect(ops.serialize().layout.children?.[1]?.id).toBe(WORKBENCH_WORKSPACE_GROUP_ID);
    expect(
      ops.openConversation({ sessionId: "conversation-1", title: "Renamed" }).panelInstanceId,
    ).toBe(first.panelInstanceId);
    expect(ops.listGroupPanels(first.groupId)).toHaveLength(2);
    ops.ensureView({ viewId: "project", title: "Project" });
    expect(ops.getPanel(first.panelInstanceId)?.visible).toBe(true);
    model.doAction(Actions.maximizeToggle(editor.groupId, WORKBENCH_WORKSPACE_LAYOUT_ID));
    expect(ops.getPanel(first.panelInstanceId)?.visible).toBe(true);
    ops.reveal(first.panelInstanceId);
    expect(model.getMaximizedTabset(WORKBENCH_WORKSPACE_LAYOUT_ID)?.getId()).toBe(editor.groupId);
    expect(ops.move({ panelInstanceId: first.panelInstanceId, groupId: editor.groupId })).toBe(
      false,
    );
    expect(
      ops.move({ panelInstanceId: editor.panelInstanceId, groupId: WORKBENCH_WORKSPACE_GROUP_ID }),
    ).toBe(false);
    expect(() =>
      ops.openEditor({
        resourceKind: "event_graph",
        resourceRef: "events/B",
        title: "B",
        mode: "new-instance",
        targetGroupId: first.groupId,
      }),
    ).toThrow("group_not_found");
    expect(ops.move({ panelInstanceId: editor.panelInstanceId, groupId: first.groupId })).toBe(
      false,
    );
    expect(ops.floatPanel(first.panelInstanceId)).toBe(false);
    expect(
      ops.split({
        panelInstanceId: first.panelInstanceId,
        referenceGroupId: editor.groupId,
        direction: "left",
      }),
    ).toBe(false);
    const restored = parsePersistedWorkbenchLayout({
      root: ops.serialize(),
      nested: { logs: DEFAULT_LOGS_LAYOUT },
    });
    expect(restored?.root.status).toBe("valid");
    if (restored?.root.status === "valid")
      expect(restored.root.value.layout.children?.[0]).toMatchObject({ enableTabStrip: false });
    const scrubbed = new WorkbenchModelOperations(
      Model.fromJson(scrubProjectScopedRootLayout(ops.serialize())),
    );
    expect(scrubbed.listPanels().some((panel) => panel.metadata.role === "conversation")).toBe(
      false,
    );
    const misplaced = ops.serialize();
    misplaced.layout.children!.reverse();
    expect(
      parsePersistedWorkbenchLayout({ root: misplaced, nested: { logs: DEFAULT_LOGS_LAYOUT } })
        ?.root.status,
    ).toBe("invalid");
    ops.removePanels([first.panelInstanceId]);
    expect(ops.getPanel(second.panelInstanceId)?.visible).toBe(true);
    ops.removePanels([second.panelInstanceId]);
    expect(model.getNodeById(WORKBENCH_CONVERSATION_GROUP_ID)).toBeUndefined();
    expect(ops.getPanel(editor.panelInstanceId)?.visible).toBe(true);
  });

  it("keeps the restored right tool selection and collapse state while installing fixed panels", async () => {
    for (const collapsed of [false, true]) {
      const model = Model.fromJson(createEmptyWorkbenchLayout());
      const ops = new WorkbenchModelOperations(model);
      const output = ops.ensureView({ viewId: "output", title: "Output" });
      ops.move({ panelInstanceId: output.panelInstanceId, groupId: "border_right" });
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
        expect(restored.getPanel(output.panelInstanceId)).toMatchObject({
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
        expect(
          restored
            .listPanels()
            .find(
              (panel) => panel.metadata.role === "view" && panel.metadata.viewId === "assistant",
            ),
        ).toMatchObject({ location: { type: "edge", position: "left" } });
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
      resourceKind: "event_graph",
      resourceRef: "events/A",
      title: "A",
      mode: "reuse-resource",
    });
    const second = ops.openEditor({
      resourceKind: "event_graph",
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
    const output = ops.ensureView({ viewId: "output", title: "Output" });
    expect(ops.getActivePanel()?.panelInstanceId).toBe(second.panelInstanceId);
    expect(ops.getPanel(first.panelInstanceId)?.visible).toBe(true);
    expect(ops.getPanel(second.panelInstanceId)?.visible).toBe(true);
    ops.reveal(details.panelInstanceId);
    expect(ops.getActivePanel()?.panelInstanceId).toBe(second.panelInstanceId);
    ops.move({ panelInstanceId: output.panelInstanceId, groupId: first.groupId });
    expect(ops.getActivePanel()?.metadata.role).toBe("view");
    ops.removePanels([output.panelInstanceId, first.panelInstanceId]);
    expect(ops.getActivePanel()?.panelInstanceId).toBe(second.panelInstanceId);
    ops.removePanels([second.panelInstanceId]);
    expect(ops.getActivePanel()).toBeUndefined();
  });

  it("keeps Assistant in the fixed Activity group through layout operations", () => {
    const model = Model.fromJson(createEmptyWorkbenchLayout());
    configureWorkbenchModel(model);
    const ops = new WorkbenchModelOperations(model);
    const editor = ops.openEditor({
      resourceKind: "event_graph",
      resourceRef: "events/A",
      title: "A",
      mode: "reuse-resource",
    });
    const assistant = ops.ensureView({ viewId: "assistant", title: "Assistant" });
    expect(model.getNodeById(assistant.panelInstanceId)?.toJson()).toMatchObject({
      enableClose: false,
      enableFloat: false,
    });
    expect(ops.move({ panelInstanceId: assistant.panelInstanceId, groupId: "border_right" })).toBe(
      false,
    );
    expect(ops.move({ panelInstanceId: assistant.panelInstanceId, groupId: editor.groupId })).toBe(
      false,
    );
    expect(
      ops.split({
        panelInstanceId: assistant.panelInstanceId,
        referenceGroupId: editor.groupId,
        direction: "left",
      }),
    ).toBe(false);
    expect(() => ops.removePanels([assistant.panelInstanceId])).toThrow();
    expect(ops.getPanel(assistant.panelInstanceId)).toMatchObject({
      location: { type: "edge", position: "left" },
    });
    expect(ops.getActivePanel()?.panelInstanceId).toBe(editor.panelInstanceId);
  });
});
