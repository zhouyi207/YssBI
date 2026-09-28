import { beforeEach, describe, expect, it } from "vitest";

import {
  buildFileResourceMeta,
  markResourceDirty,
  useDocumentStateStore,
  useResourceStore,
} from "@/features/core/resource";
import { collectDirtyEditorPanels } from "./editorPanelDirty";

describe("collectDirtyEditorPanels", () => {
  beforeEach(() => {
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().clear();
  });

  it("does not infer an open panel from dirty document state", () => {
    const path = "events/A.yssbi-event";
    useResourceStore.getState().upsertResource(buildFileResourceMeta("event_graph", path, "A"));
    markResourceDirty({ id: path, kind: "event_graph" }, true);

    expect(collectDirtyEditorPanels()).toEqual([]);
  });
});
