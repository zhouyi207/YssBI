import { Actions } from "flexlayout-react";
import { describe, expect, it } from "vitest";
import { LayoutModelBinding } from "./layoutModelBinding";
import { configureWorkbenchModel } from "./workbenchActivityGroup";
import { createEmptyWorkbenchLayout } from "./workbenchLayoutDefaults";
import { createWorkbenchLayoutRuntime } from "./workbenchLayoutInternal";
import { WorkbenchModelOperations } from "./workbenchLayoutOperations";
import { PendingWorkbenchTransaction } from "./workbenchLayoutTransaction";

function createRuntime() {
  const binding = new LayoutModelBinding(createEmptyWorkbenchLayout(), configureWorkbenchModel);
  const runtime = createWorkbenchLayoutRuntime();
  runtime.internal.bind(binding);
  runtime.internal.completeHydration();
  return { binding, ...runtime, ops: new WorkbenchModelOperations(binding.getModel()) };
}

describe("workbench notification boundaries", () => {
  it("routes committed geometry, panel data, selection and membership to their actual consumers", () => {
    const { binding, read, internal, ops } = createRuntime();
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
    const settings = ops.ensureView({ viewId: "settings", title: "Settings" });
    const details = ops.ensureView({ viewId: "details", title: "Details" });
    const assistant = ops.ensureView({ viewId: "assistant", title: "Assistant" });
    ops.reveal(details.panelInstanceId);
    ops.activate(first.panelInstanceId);
    const calls = {
      semantic: 0,
      persistence: 0,
      active: 0,
      members: 0,
      first: 0,
      second: 0,
      assistant: 0,
      model: 0,
    };
    read.subscribe(() => calls.semantic++);
    read.subscribePersistence(() => calls.persistence++);
    read.subscribeActivePanel(() => calls.active++);
    read.subscribePanelSet(() => calls.members++);
    read.subscribePanel(first.panelInstanceId, () => calls.first++);
    read.subscribePanel(second.panelInstanceId, () => calls.second++);
    read.subscribePanel(assistant.panelInstanceId, () => calls.assistant++);
    binding.subscribeModel(() => calls.model++);
    const before = read.getSnapshot();
    const activeBefore = read.getActiveSnapshot();

    ops.setEdgeSize("right", 480);
    expect(calls).toEqual({
      semantic: 0,
      persistence: 1,
      active: 0,
      members: 0,
      first: 0,
      second: 0,
      assistant: 0,
      model: 0,
    });
    expect(read.getSnapshot()).toBe(before);
    binding.getModel().doAction(Actions.renameTab(second.panelInstanceId, "B updated"));
    expect(calls).toMatchObject({
      semantic: 1,
      persistence: 2,
      active: 0,
      members: 0,
      first: 0,
      second: 1,
      model: 0,
    });
    expect(read.getActiveSnapshot()).toBe(activeBefore);
    ops.setEdgeCollapsed("right", true);
    // Even an unselected border tab's context menu observes the edge's collapse state.
    expect(calls.assistant).toBe(1);
    expect(calls.active).toBe(0);
    ops.activate(second.panelInstanceId);
    expect(calls).toMatchObject({ active: 1, members: 0, first: 1, second: 2 });
    expect(read.getActiveSnapshot()).not.toBe(activeBefore);
    ops.removePanels([settings.panelInstanceId]);
    expect(calls.members).toBe(1);
    const semanticCount = calls.semantic;
    binding.replace(ops.serialize());
    expect(calls.model).toBe(1);
    expect(calls.semantic).toBe(semanticCount);
    internal.unbind(binding);
    expect(read.getSnapshot()).toMatchObject({ ready: false, hydrated: false });
    expect(read.getActiveSnapshot()).toMatchObject({ ready: false, hydrated: false });
    expect(calls.members).toBe(2);
  });

  it("defers any adjusting action and nested gesture batch without an action-name allowlist", () => {
    const { binding, read, internal, ops } = createRuntime();
    const panel = ops.ensureView({ viewId: "settings", title: "Settings" });
    const model = binding.getModel();
    const pending = new PendingWorkbenchTransaction(binding);
    const revision = binding.getSnapshot().revision;
    const mutationRevision = read.getMutationRevision();
    let semantic = 0;
    let persisted = 0;
    read.subscribe(() => semantic++);
    read.subscribePersistence(() => persisted++);
    model.doAction(
      Actions.updateNodeAttributes(panel.panelInstanceId, { name: "Intermediate" }).setAdjusting(
        true,
      ),
    );
    model.doAction(
      Actions.group([
        Actions.group([
          Actions.updateNodeAttributes(panel.panelInstanceId, {
            name: "Last intermediate",
          }).setAdjusting(true),
        ]),
      ]),
    );
    expect(model.getNodeById(panel.panelInstanceId)?.getAttributeOwn("name")).toBe(
      "Last intermediate",
    );
    expect(read.getPanel(panel.panelInstanceId)?.title).toBe("Settings");
    expect(binding.getSnapshot().revision).toBe(revision + 2);
    expect(read.getMutationRevision()).not.toBe(mutationRevision);
    expect(() => pending.commit()).toThrow("layout_restore_failed");
    expect([semantic, persisted]).toEqual([0, 0]);
    model.doAction(Actions.updateNodeAttributes(panel.panelInstanceId, { name: "Committed" }));
    expect([semantic, persisted]).toEqual([1, 1]);
    internal.unbind(binding);
  });
});
