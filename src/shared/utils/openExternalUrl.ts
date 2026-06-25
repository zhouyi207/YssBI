import { openUrl } from "@tauri-apps/plugin-opener";
import { uiStore } from "@/features/core/ui/UIStore";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";

export async function openExternalUrl(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch (error) {
    uiStore.showToast(formatErrorMessage(error), "error", 4000);
  }
}
