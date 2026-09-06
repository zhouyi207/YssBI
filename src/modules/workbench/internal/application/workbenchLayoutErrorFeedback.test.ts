import { beforeEach, describe, expect, it, vi } from "vitest";

const alert = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));

vi.mock("i18next", () => ({
  default: { t: (key: string) => `translated:${key}` },
}));

vi.mock("@/features/core/ui/UIStore", () => ({
  uiStore: { alert },
}));

import { showWorkbenchLayoutError } from "./workbenchLayoutErrorFeedback";

beforeEach(() => {
  alert.mockClear();
});

describe("workbench layout error feedback", () => {
  it("uses generic localized feedback without exposing raw exception text", () => {
    showWorkbenchLayoutError(new Error("private Dockview exception text"));

    expect(alert).toHaveBeenCalledOnce();
    expect(alert).toHaveBeenCalledWith({
      title: "translated:common.error",
      message: "translated:workbench.layoutError.openFailed",
      closeText: "translated:common.close",
      type: "error",
    });
    expect(JSON.stringify(alert.mock.calls[0]?.[0])).not.toContain(
      "private Dockview exception text",
    );
  });
});
