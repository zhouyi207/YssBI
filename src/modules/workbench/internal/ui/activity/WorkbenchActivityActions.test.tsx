// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { IDockviewHeaderActionsProps } from "dockview-react";

import { TooltipProvider } from "@/components/ui/tooltip";
import { workbenchUi } from "../../state/ui";
const mocks = vi.hoisted(() => ({ reveal: vi.fn() }));
vi.mock("../../application/workbenchLayoutActions", () => ({ revealWorkbenchView: mocks.reveal }));
vi.mock("../../dockview/workbenchRead", () => ({
  workbenchDockviewRead: {
    subscribe: () => () => {},
    getEdgeState: () => ({ groupId: "left", visible: true, collapsed: false }),
    listGroups: () => [{ groupId: "left", activePanelInstanceId: "plugins" }],
    listPanels: () => [
      { panelInstanceId: "plugins", metadata: { role: "view", viewId: "plugins" }, visible: true },
    ],
  },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { WorkbenchActivityActions } from "./WorkbenchActivityActions";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function props(groupId: string): IDockviewHeaderActionsProps {
  return {
    group: { id: groupId } as IDockviewHeaderActionsProps["group"],
    headerPosition: "left",
  } as IDockviewHeaderActionsProps;
}

describe("WorkbenchActivityActions", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    workbenchUi.setSettingsOpen(false);
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    document.body.replaceChildren();
  });

  it("opens the plugin panel from the bottom Activity action and reflects Dockview selection", () => {
    act(() =>
      root.render(
        <TooltipProvider>
          <WorkbenchActivityActions
            {...props("workbench-edge-left")}
            additionalActions={<span data-testid="additional-action" />}
          />
        </TooltipProvider>,
      ),
    );

    const plugins = host.querySelector<HTMLElement>("[data-workbench-activity-plugins]");
    expect(plugins?.tagName).toBe("BUTTON");
    expect(plugins?.getAttribute("aria-pressed")).toBe("true");
    expect(host.querySelector("[data-testid='additional-action']")).not.toBeNull();

    act(() => plugins?.click());
    expect(mocks.reveal).toHaveBeenCalledWith("plugins");
    expect(workbenchUi.getSnapshot().isSettingsOpen).toBe(false);

    act(() =>
      root.render(
        <TooltipProvider>
          <WorkbenchActivityActions {...props("ordinary-group")} />
        </TooltipProvider>,
      ),
    );
    expect(host.querySelector("[data-workbench-activity-plugins]")).toBeNull();
    expect(host.querySelector("[data-testid='additional-action']")).toBeNull();
  });
});
