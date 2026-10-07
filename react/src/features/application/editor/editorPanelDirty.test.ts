import { beforeEach, describe, expect, it } from "vitest";

import {
  buildFileResourceMeta,
  markResourceDirty,
  useResourceStore,
} from "@/features/core/resource";
import { collectDirtyEditorPanels } from "./editorPanelDirty";

describe("collectDirtyEditorPanels", () => {
  beforeEach(() => {
    useResourceStore.getState().clear();
  });

  it("does not infer an open panel from dirty document state", () => {
    const path = "events/A.yssbi-event";
    useResourceStore
      .getState()
      .setSnapshot({ resources: [buildFileResourceMeta("event_graph", path, "A")] });
    markResourceDirty({ id: path, kind: "event_graph" }, true);

    expect(collectDirtyEditorPanels()).toEqual([]);
  });
});
