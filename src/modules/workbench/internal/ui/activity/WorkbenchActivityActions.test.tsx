// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { IDockviewHeaderActionsProps } from "dockview-react";

import { TooltipProvider } from "@/components/ui/tooltip";

const mocks = vi.hoisted(() => ({
  openSettings: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("../../state/ui", () => ({
  workbenchUi: {
    openSettings: mocks.openSettings,
  },
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
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    document.body.replaceChildren();
  });

  it("renders Settings only for the Activity group and opens the settings dialog", () => {
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

    const settingsButton = host.querySelector<HTMLButtonElement>(
      "[data-workbench-activity-settings]",
    );
    expect(settingsButton).not.toBeNull();

    act(() => settingsButton?.click());
    expect(mocks.openSettings).toHaveBeenCalledOnce();

    act(() =>
      root.render(
        <TooltipProvider>
          <WorkbenchActivityActions {...props("ordinary-group")} />
        </TooltipProvider>,
      ),
    );
    expect(host.querySelector("[data-workbench-activity-settings]")).toBeNull();
    expect(host.querySelector("[data-testid='additional-action']")).toBeNull();
  });
});
