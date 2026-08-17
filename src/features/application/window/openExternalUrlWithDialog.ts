import type { TFunction } from "i18next";
import { uiStore } from "@/features/core/ui/UIStore";
import { openExternalUrl } from "@/shared/utils/openExternalUrl";

export async function openExternalUrlWithDialog(
  url: string,
  t: TFunction,
): Promise<void> {
  try {
    await openExternalUrl(url);
  } catch {
    await uiStore.alert({
      title: t("common.error"),
      message: t("notifications.externalUrl.openFailed"),
      closeText: t("common.close"),
      type: "error",
    });
  }
}
