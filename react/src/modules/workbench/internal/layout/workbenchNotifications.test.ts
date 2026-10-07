import { Actions, DockLocation } from "flexlayout-react";
import { describe, expect, it } from "vitest";
import { LayoutModelBinding } from "./layoutModelBinding";
import { configureWorkbenchModel } from "./workbenchActivityGroup";
import {
  createEmptyWorkbenchLayout,
  WORKBENCH_WORKSPACE_LAYOUT_ID,
} from "./workbenchLayoutDefaults";
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
  it("publishes only the completed panel when opening or revealing an editor", () => {
    const { read, internal, ops } = createRuntime();
    const first = ops.openEditor({
      resourceKind: "event_graph",
      resourceRef: "events/A",
      title: "A",
      mode: "reuse-resource",
    });
    const observed: { active: string | undefined; title: string | undefined; panels: number }[] =
      [];
    const unsubscribe = read.subscribePersistence(() => {
      observed.push({
        active: read.getActivePanel()?.panelInstanceId,
        title: read.getActivePanel()?.title,
        panels: read.listPanels().length,
      });
    });
    try {
      const second = ops.openEditor({
        resourceKind: "event_graph",
        resourceRef: "events/B",
        title: "B",
        mode: "reuse-resource",
      });
      expect(observed).toEqual([{ active: second.panelInstanceId, title: "B", panels: 2 }]);
      observed.length = 0;
      const reused = ops.openEditor({
        resourceKind: "event_graph",
        resourceRef: "events/A",
        title: "A renamed",
        mode: "reuse-resource",
      });
      expect(reused.panelInstanceId).toBe(first.panelInstanceId);
      expect(observed).toEqual([{ active: first.panelInstanceId, title: "A renamed", panels: 2 }]);
    } finally {
      unsubscribe();
      internal.unbind();
    }
  });

  it("commits a native move into a float with that panel as the only active target", () => {
    const { binding, read, internal, ops } = createRuntime();
    const open = (name: string) =>
      ops.openEditor({
        resourceKind: "event_graph",
        resourceRef: `events/${name}`,
        title: name,
        mode: "reuse-resource",
      });
    const first = open("A");
    open("B");
    const floating = open("C");
    ops.floatPanel(floating.panelInstanceId);
    const target = ops.getPanel(floating.panelInstanceId)!;
    ops.activate(first.panelInstanceId);
    const observed: (string | undefined)[] = [];
    const unsubscribe = read.subscribePersistence(() =>
      observed.push(read.getActivePanel()?.panelInstanceId),
    );
    try {
      binding
        .getModel()
        .doAction(
          Actions.moveNode(first.panelInstanceId, target.groupId, DockLocation.CENTER, -1, true),
        );
      expect(observed).toEqual([first.panelInstanceId]);
      expect(read.getPanel(first.panelInstanceId)?.groupId).toBe(target.groupId);
      expect(
        binding.getModel().getActiveTabset(WORKBENCH_WORKSPACE_LAYOUT_ID)?.getId(),
      ).toBeUndefined();
    } finally {
      unsubscribe();
      internal.unbind();
    }
  });

  it("routes committed geometry, panel data, selection and membership to their actual consumers", () => {
    const { binding, read, internal, ops } = createRuntime();
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
    const logs = ops.ensureView({ viewId: "logs", title: "Logs" });
    const details = ops.ensureView({ viewId: "details", title: "Details" });
    const assistant = ops.ensureView({ viewId: "assistant", title: "Assistant" });
    ops.ensureView({ viewId: "project", title: "Project" });
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
    expect(calls.assistant).toBe(0);
    ops.setEdgeCollapsed("left", true);
    // Even an unselected border tab's context menu observes the edge's collapse state.
    expect(calls.assistant).toBe(1);
    expect(calls.active).toBe(0);
    ops.activate(second.panelInstanceId);
    expect(calls).toMatchObject({ active: 1, members: 0, first: 1, second: 2 });
    expect(read.getActiveSnapshot()).not.toBe(activeBefore);
    ops.removePanels([logs.panelInstanceId]);
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
    const panel = ops.ensureView({ viewId: "logs", title: "Logs" });
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
    expect(read.getPanel(panel.panelInstanceId)?.title).toBe("Logs");
    expect(binding.getSnapshot().revision).toBe(revision + 2);
    expect(read.getMutationRevision()).not.toBe(mutationRevision);
    expect(() => pending.commit()).toThrow("layout_restore_failed");
    expect([semantic, persisted]).toEqual([0, 0]);
    model.doAction(Actions.updateNodeAttributes(panel.panelInstanceId, { name: "Committed" }));
    expect([semantic, persisted]).toEqual([1, 1]);
    internal.unbind(binding);
  });

  it("preserves result metadata on model replacement while publishing reference and lease changes", () => {
    const { binding, read, internal, ops } = createRuntime();
    const panel = ops.upsertResult({
      reference: { executionSessionId: "00000000-0000-0000-0000-000000000091", resultId: "1" },
      leaseId: "00000000-0000-0000-0000-000000000092",
      title: "Result",
      presentation: { kind: "inspector" },
    });
    const previous = read.getPanel(panel.panelInstanceId)!;
    const groups = read.listGroups();
    let members = 0;
    const unsubscribe = read.subscribePanelSet(() => members++);
    try {
      binding.replace(ops.serialize());
      expect(read.getPanel(panel.panelInstanceId)).toBe(previous);
      expect(read.listGroups()).toBe(groups);
      expect(members).toBe(0);
      if (previous.metadata.role !== "result") throw new Error("Expected result metadata");

      const currentOps = new WorkbenchModelOperations(binding.getModel());
      currentOps.replaceResult(previous.metadata.reference, {
        reference: { ...previous.metadata.reference, resultId: "2" },
        leaseId: "00000000-0000-0000-0000-000000000093",
        title: "New result",
        presentation: { ...previous.metadata.presentation },
      });
      const changed = read.getPanel(panel.panelInstanceId)!;
      expect(changed.metadata).toMatchObject({
        reference: { resultId: "2" },
        leaseId: "00000000-0000-0000-0000-000000000093",
        title: "New result",
      });
      expect(members).toBe(1);
      if (changed.metadata.role !== "result") throw new Error("Expected result metadata");
      expect(changed.metadata.presentation).toBe(previous.metadata.presentation);
      expect(changed.location).toBe(previous.location);
      expect(read.listGroups()).toBe(groups);
      expect(previous.metadata.reference.resultId).toBe("1");

      currentOps.removePanels([panel.panelInstanceId]);
      expect(read.getPanel(panel.panelInstanceId)).toBeUndefined();
      expect(members).toBe(2);
    } finally {
      unsubscribe();
      internal.unbind(binding);
    }
  });
});
