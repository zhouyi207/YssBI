import { logger } from "@/utils/appLogger";
import { openUrl } from "@tauri-apps/plugin-opener";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";

export async function openExternalUrl(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch (error) {
    logger.notify.error(formatErrorMessage(error), "UI");
  }
}
