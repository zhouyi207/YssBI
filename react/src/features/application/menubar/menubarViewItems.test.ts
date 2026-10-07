import { describe, expect, it, vi } from "vitest";
import {
  buildViewMenuItems,
  type MenubarViewMenuActions,
  type MenubarViewState,
} from "./menubarViewItems";

const t = ((key: string) => key) as never;

function actions(): MenubarViewMenuActions {
  return {
    toggleActivityGroup: vi.fn(),
    toggleAssistant: vi.fn(),
    resetLayout: vi.fn(),
  };
}

function state(overrides: Partial<MenubarViewState> = {}): MenubarViewState {
  return {
    activityGroupOpen: true,
    assistantOpen: true,
    ...overrides,
  };
}

describe("buildViewMenuItems", () => {
  it("exposes sidebar toggles and layout reset", () => {
    const callbacks = actions();
    const items = buildViewMenuItems(t, state(), callbacks);

    expect(items.map((item) => item.label)).toEqual([
      "panel.primarySideBar",
      "panel.assistant",
      "-",
      "menubar.resetLayout",
    ]);
    expect(items[0]).toMatchObject({
      type: "checkbox",
      checked: true,
      onClick: callbacks.toggleActivityGroup,
    });
    expect(items[1]).toMatchObject({
      type: "checkbox",
      checked: true,
      onClick: callbacks.toggleAssistant,
    });
    expect(items[3]?.onClick).toBe(callbacks.resetLayout);
  });
});
