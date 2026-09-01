import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  WorkbenchLayoutError,
  type WorkbenchLayoutErrorCode,
} from "@/features/core/dockview/workbenchTypes";

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
  it("maps every stable layout error code to its localized message", () => {
    const cases: ReadonlyArray<readonly [WorkbenchLayoutErrorCode, string]> = [
      ["dockview_not_ready", "workbench.layoutError.notReady"],
      ["invalid_panel_metadata", "workbench.layoutError.invalidPanel"],
      ["group_not_found", "workbench.layoutError.groupUnavailable"],
      ["panel_open_failed", "workbench.layoutError.openFailed"],
      ["layout_restore_failed", "workbench.layoutError.restoreFailed"],
    ];

    for (const [code, messageKey] of cases) {
      showWorkbenchLayoutError(new WorkbenchLayoutError(code));
      expect(alert).toHaveBeenLastCalledWith({
        title: "translated:common.error",
        message: `translated:${messageKey}`,
        closeText: "translated:common.close",
        type: "error",
      });
    }
    expect(alert).toHaveBeenCalledTimes(cases.length);
  });

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
