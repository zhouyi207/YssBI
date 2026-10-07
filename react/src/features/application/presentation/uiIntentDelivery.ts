import { pendingUiIntents, settleUiIntent } from "@/services/workbench/presentationService";
import type { UiIntent, UiIntentReceipt } from "@/shared/types/domain/uiPresentation";

/** Transient delivery of backend-owned receipts, scoped to one workbench binding. */
export function createUiIntentDelivery(
  projectInstanceId: string,
  current: () => boolean,
  execute: (intent: UiIntent) => Promise<boolean>,
  failed: () => void,
) {
  let queue = Promise.resolve();
  let resync: Promise<void> | null = null;
  let recoverAfterFlight = false;
  let recoverAfterQueue = false;
  const queued = new Set<string>();
  const accept = (receipt: UiIntentReceipt): Promise<void> => {
    if (receipt.status !== "pending" || !current() || queued.has(receipt.id))
      return Promise.resolve();
    if (queued.size >= 128) {
      recoverAfterQueue = true;
      return Promise.resolve();
    }
    queued.add(receipt.id);
    queue = queue
      .then(async () => {
        if (!current() || !(await settleUiIntent(projectInstanceId, receipt.id, "claimed"))) return;
        let applied = false;
        try {
          applied = await execute(receipt.intent);
        } finally {
          if (current())
            await settleUiIntent(projectInstanceId, receipt.id, applied ? "applied" : "failed");
        }
      })
      .catch(failed)
      .finally(() => {
        queued.delete(receipt.id);
        if (queued.size === 0 && recoverAfterQueue) {
          recoverAfterQueue = false;
          if (current()) void recover(true);
        }
      });
    return queue;
  };
  const recover = (fresh = false): Promise<void> => {
    if (!current()) return Promise.resolve();
    if (resync) {
      recoverAfterFlight ||= fresh;
      return resync;
    }
    recoverAfterFlight = false;
    resync = pendingUiIntents(projectInstanceId)
      .then((receipts) => {
        if (current()) receipts.forEach((receipt) => void accept(receipt));
      })
      .catch(failed)
      .finally(() => {
        resync = null;
        if (recoverAfterFlight && current()) void recover();
      });
    return resync;
  };
  return { accept, recover };
}
