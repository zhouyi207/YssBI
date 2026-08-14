import { logger } from "@/utils/appLogger";
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { activateCurrentEditorTab } from './switchEditorTab';

const DEFAULT_MAX_ATTEMPTS = 3;
const DEFAULT_RETRY_DELAY_MS = 150;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

export type BootstrapEditorGraphSessionOptions = {
  maxAttempts?: number;
  retryDelayMs?: number;
};

/** Bind graph session for the active editor tab after workbench hydrate (retries transient load failures). */
export async function bootstrapEditorGraphSession(
  groupId: string,
  options?: BootstrapEditorGraphSessionOptions,
): Promise<boolean> {
  const maxAttempts = options?.maxAttempts ?? DEFAULT_MAX_ATTEMPTS;
  const retryDelayMs = options?.retryDelayMs ?? DEFAULT_RETRY_DELAY_MS;
  if (!getActiveLayoutTab(groupId)) return true;

  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    const loaded = await activateCurrentEditorTab(groupId);
    if (loaded) return true;
    if (attempt < maxAttempts) {
      await sleep(retryDelayMs);
    }
  }

  logger.notify.warn('当前编辑器图未能加载，请重新点击标签页或画布', "UI");
  return false;
}
