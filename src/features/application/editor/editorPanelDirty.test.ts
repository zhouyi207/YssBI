import { beforeEach, describe, expect, it } from "vitest";

import {
  buildGraphResourceMeta,
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
    useResourceStore.getState().upsertResource(buildGraphResourceMeta("event", path, "A"));
    markResourceDirty({ id: path, kind: "event" }, true);

    expect(collectDirtyEditorPanels()).toEqual([]);
  });
});
