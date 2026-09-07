// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { IDockviewHeaderActionsProps } from "dockview-react";

import { TooltipProvider } from "@/components/ui/tooltip";
import { workbenchUi } from "../../state/ui";

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

  it("renders an inert plugin placeholder only for the Activity group", () => {
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
    expect(plugins).not.toBeNull();
    expect(plugins?.getAttribute("role")).toBe("img");
    expect(host.querySelector("button")).toBeNull();

    act(() => plugins?.click());
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
