// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useSidebarResourceActions } from "./useSidebarResourceActions";

const mocks = vi.hoisted(() => ({
  addVariable: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("@/features/application/editor", () => ({
  useEditorSessionCommandsContext: () => ({
    addVariable: mocks.addVariable,
  }),
  useEditorSessionResources: () => ({
    events: {},
    functions: {},
  }),
}));

vi.mock("@/features/application/window", () => ({
  openDatabaseEditorWindow: vi.fn(),
}));

vi.mock("@/features/core/editor", () => ({
  useFunctionCatalog: () => ({}),
}));

vi.mock("@/features/core/graphSession/graphSessionStore", () => ({
  useGraphSessionStore: Object.assign(
    (selector: (state: { focusedSession: { groupId: string; graphPath: string } }) => unknown) =>
      selector({
        focusedSession: {
          groupId: "group-1",
          graphPath: "charts/Report.yssbi-chart",
        },
      }),
    {
      getState: () => ({
        focusedSession: {
          groupId: "group-1",
          graphPath: "charts/Report.yssbi-chart",
        },
      }),
      subscribe: () => () => {},
    },
  ),
}));

vi.mock("@/features/core/dockview", () => ({
  workbenchDockviewRead: {
    getActiveEditorPanelInGroup: () => ({
      metadata: {
        role: "editor",
        resourceRef: "charts/Report.yssbi-chart",
        resourceKind: "chart",
      },
    }),
  },
}));

vi.mock("@/features/core/resource", () => ({
  useGraphResourcesByKind: () => ({}),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("useSidebarResourceActions", () => {
  let root: Root;
  let host: HTMLDivElement;
  let actions: ReturnType<typeof useSidebarResourceActions>;

  beforeEach(() => {
    vi.clearAllMocks();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    function Harness() {
      actions = useSidebarResourceActions(vi.fn());
      return null;
    }
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("does not expose local Project actions for a chart active tab", async () => {
    expect(actions.canDemoteVariable).toBe(false);

    await act(async () => {
      await actions.addVariable("Threshold", "Int64", false);
    });

    expect(mocks.addVariable).not.toHaveBeenCalled();
  });
});
