import type { TFunction } from "i18next";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { openExternalUrlWithDialog } from "./openExternalUrlWithDialog";

const openExternalUrl = vi.hoisted(() => vi.fn());
const alert = vi.hoisted(() => vi.fn());

vi.mock("@/shared/utils/openExternalUrl", () => ({ openExternalUrl }));
vi.mock("@/features/core/ui/UIStore", () => ({
  uiStore: { alert },
}));

const t = ((key: string) => `localized:${key}`) as TFunction;

describe("openExternalUrlWithDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it("opens the URL without showing a dialog", async () => {
    openExternalUrl.mockResolvedValueOnce(undefined);

    await expect(openExternalUrlWithDialog("https://example.com", t)).resolves.toBeUndefined();

    expect(openExternalUrl).toHaveBeenCalledWith("https://example.com");
    expect(alert).not.toHaveBeenCalled();
  });

  it("shows a localized dialog without exposing the opener error", async () => {
    openExternalUrl.mockRejectedValueOnce(new Error("sensitive native opener failure"));
    alert.mockResolvedValueOnce(undefined);

    await expect(openExternalUrlWithDialog("https://example.com", t)).resolves.toBeUndefined();

    expect(alert).toHaveBeenCalledWith({
      title: "localized:common.error",
      message: "localized:notifications.externalUrl.openFailed",
      closeText: "localized:common.close",
      type: "error",
    });
    expect(JSON.stringify(alert.mock.calls)).not.toContain("sensitive native opener failure");
  });
});
