import { logger } from "@/utils/appLogger";
import { openUrl } from "@tauri-apps/plugin-opener";

export async function openExternalUrl(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch (error) {
    logger.app.error(
      `External URL open failed: ${error instanceof Error ? error.message : String(error)}`,
      "openExternalUrl",
    );
    throw error;
  }
}
