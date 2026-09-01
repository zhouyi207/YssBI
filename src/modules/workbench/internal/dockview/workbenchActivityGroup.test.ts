import { describe, expect, it, vi } from "vitest";

import {
  canMoveWorkbenchPanel,
  canRemoveWorkbenchPanel,
  canSplitWorkbenchPanel,
  shouldAllowWorkbenchActivityDrop,
  vetoInvalidWorkbenchActivityDrop,
  WORKBENCH_ACTIVITY_GROUP_ID,
  type WorkbenchActivityDropEvent,
} from "./workbenchActivityGroup";
import type { WorkbenchPanelMetadata } from "./workbenchPanelModel";

const activityMetadata: WorkbenchPanelMetadata = { role: "view", viewId: "project" };
const editorMetadata: WorkbenchPanelMetadata = {
  role: "editor",
  resourceRef: "events/Main.yssbi-event",
  resourceKind: "event",
};
const detailsMetadata: WorkbenchPanelMetadata = { role: "view", viewId: "details" };

function dropEvent({
  sourceMetadata,
  sourceGroupId,
  targetGroupId,
  kind = "tab",
}: {
  sourceMetadata: WorkbenchPanelMetadata;
  sourceGroupId: string;
  targetGroupId: string;
  kind?: "tab" | "header_space";
}): WorkbenchActivityDropEvent {
  const source = { params: { metadata: sourceMetadata } };
  const preventDefault = vi.fn();
  return {
    api: {
      id: "root",
      getPanel: vi.fn(() => source),
    },
    group: { id: targetGroupId },
    panel: { group: { id: targetGroupId } },
    kind,
    getData: () => ({
      viewId: "root",
      groupId: sourceGroupId,
      panelId: "source-panel",
    }),
    preventDefault,
  } as unknown as WorkbenchActivityDropEvent;
}

describe("workbench Activity group policy", () => {
  it("allows native Activity reorder but blocks cross-group moves, split, and close", () => {
    const reorder = dropEvent({
      sourceMetadata: activityMetadata,
      sourceGroupId: WORKBENCH_ACTIVITY_GROUP_ID,
      targetGroupId: WORKBENCH_ACTIVITY_GROUP_ID,
    });
    const outbound = dropEvent({
      sourceMetadata: activityMetadata,
      sourceGroupId: WORKBENCH_ACTIVITY_GROUP_ID,
      targetGroupId: "workbench-edge-right",
    });
    const inbound = dropEvent({
      sourceMetadata: editorMetadata,
      sourceGroupId: "grid-main",
      targetGroupId: WORKBENCH_ACTIVITY_GROUP_ID,
    });
    const persistentOutbound = dropEvent({
      sourceMetadata: detailsMetadata,
      sourceGroupId: "workbench-edge-right",
      targetGroupId: "grid-main",
    });

    expect(shouldAllowWorkbenchActivityDrop(reorder)).toBe(true);
    expect(shouldAllowWorkbenchActivityDrop(outbound)).toBe(false);
    expect(shouldAllowWorkbenchActivityDrop(inbound)).toBe(false);
    expect(shouldAllowWorkbenchActivityDrop(persistentOutbound)).toBe(false);
    vetoInvalidWorkbenchActivityDrop(outbound);
    expect(outbound.preventDefault).toHaveBeenCalledOnce();

    expect(canMoveWorkbenchPanel(activityMetadata, WORKBENCH_ACTIVITY_GROUP_ID)).toBe(true);
    expect(canMoveWorkbenchPanel(activityMetadata, "grid-main")).toBe(false);
    expect(canMoveWorkbenchPanel(detailsMetadata, "workbench-edge-right")).toBe(true);
    expect(canMoveWorkbenchPanel(detailsMetadata, "grid-main")).toBe(false);
    expect(canMoveWorkbenchPanel(editorMetadata, WORKBENCH_ACTIVITY_GROUP_ID)).toBe(false);
    expect(canSplitWorkbenchPanel(activityMetadata, "grid-main")).toBe(false);
    expect(canSplitWorkbenchPanel(detailsMetadata, "workbench-edge-right")).toBe(false);
    expect(canSplitWorkbenchPanel(editorMetadata, WORKBENCH_ACTIVITY_GROUP_ID)).toBe(false);
    expect(canRemoveWorkbenchPanel(activityMetadata)).toBe(false);
    expect(canRemoveWorkbenchPanel(detailsMetadata)).toBe(false);
    expect(canRemoveWorkbenchPanel(editorMetadata)).toBe(true);
  });
});
