// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useEditorStore } from "@/features/core/editor";
import { PROJECT_TREE_EXPANSION_DEFAULTS, useSidebarStore } from "@/features/core/sidebar";
import { useVariableManagement } from "./useVariableManagement";

const mocks = vi.hoisted(() => ({
  createVariableAction: vi.fn(),
  revealWorkbenchView: vi.fn(),
}));

vi.mock("./variableActions", () => ({
  createVariableAction: mocks.createVariableAction,
}));

vi.mock("@/modules/workbench/internal/application/workbenchLayoutActions", () => ({
  revealWorkbenchView: mocks.revealWorkbenchView,
}));

vi.mock("@/features/core/editor/hooks/useActiveEditorGroup", () => ({
  useActiveEditorGroup: () => ({
    activeResourceRef: "functions/Detail.yssbi-function",
    panels: [
      {
        metadata: {
          role: "editor",
          resourceRef: "functions/Detail.yssbi-function",
          resourceKind: "function",
        },
      },
    ],
  }),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("useVariableManagement", () => {
  let root: Root;
  let host: HTMLDivElement;
  let actions: ReturnType<typeof useVariableManagement>;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.createVariableAction.mockResolvedValue("variable-1");
    useEditorStore.setState({ variablesGraphScopePath: "functions/Detail.yssbi-function" });
    useSidebarStore.setState({
      projectTreeExpandedCategories: { ...PROJECT_TREE_EXPANSION_DEFAULTS },
    });
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    function Harness() {
      actions = useVariableManagement();
      return null;
    }
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("uses an explicit Project graph scope over the remembered Detail scope", async () => {
    const scope = {
      graphPath: "events/Main.yssbi-event",
      graphType: "event" as const,
    };

    await act(async () => {
      await actions.addVariable("Threshold", "Int64", false, { graphScope: scope });
    });

    expect(mocks.createVariableAction).toHaveBeenCalledWith(
      expect.objectContaining({
        activeGraphPath: scope.graphPath,
        graphType: scope.graphType,
        isGlobal: false,
      }),
    );
    expect(mocks.revealWorkbenchView).toHaveBeenCalledWith("project");
    expect(useSidebarStore.getState().projectTreeExpandedCategories).toMatchObject({
      "project.variables": true,
      "project.localVariables": true,
    });
  });

  it("expands the Variables and global folders for a global variable", async () => {
    await act(async () => {
      await actions.addVariable("Theme", "String", true);
    });

    expect(useSidebarStore.getState().projectTreeExpandedCategories).toMatchObject({
      "project.variables": true,
      "project.globalVariables": true,
    });
  });
});
