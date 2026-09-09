// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { DockviewReact, type DockviewApi } from "dockview-react";
import { afterEach, describe, expect, it } from "vitest";
import { workbenchDockviewInternal } from "@/modules/workbench/internal/dockview/workbenchDockviewInternal";
import { useResourceStore } from "@/features/core/resource";
import { editorUi } from "@/features/core/editor/ui";
import { openDatabaseInEditor } from "./openDatabaseInEditor";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("openDatabaseInEditor", () => {
  let root: Root;
  let api: DockviewApi;
  let host: HTMLDivElement;

  afterEach(() => {
    act(() => {
      workbenchDockviewInternal.unbind(api);
      root.unmount();
    });
    host.remove();
    useResourceStore.getState().clear();
    editorUi.clearDetailFocus();
  });

  it("opens and reuses a named data tab in the main editor group", async () => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    useResourceStore.getState().upsertResource({
      id: "sales",
      kind: "database",
      name: "Sales",
      uri: "yssbi://database/sales",
      exists: true,
      loaded: true,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
    await act(async () =>
      root.render(
        <DockviewReact
          components={{ EditorResource: () => null }}
          onReady={({ api: readyApi }) => {
            api = readyApi;
            api.addPanel({
              id: "chart",
              component: "EditorResource",
              params: {
                metadata: { role: "editor", resourceKind: "chart", resourceRef: "charts/Report" },
              },
            });
            workbenchDockviewInternal.bind(api);
            workbenchDockviewInternal.completeHydration();
          }}
        />,
      ),
    );
    const mainGroup = api.getPanel("chart")!.group;

    await act(async () => openDatabaseInEditor("sales"));
    const dataPanel = api.activePanel!;
    expect(dataPanel.group).toBe(mainGroup);
    expect(dataPanel.title).toBe("Sales");
    expect(dataPanel.params?.metadata).toMatchObject({
      role: "editor",
      resourceKind: "database",
      resourceRef: "sales",
    });
    expect(editorUi.getSnapshot().detailFocus).toEqual({ kind: "data", id: "sales" });

    await act(async () => openDatabaseInEditor("sales"));
    expect(api.activePanel).toBe(dataPanel);
    expect(api.panels).toHaveLength(2);
  });
});
