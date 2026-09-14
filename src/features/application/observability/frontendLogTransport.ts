import { LogService } from "@/services/log";
import { createFrontendLogBatcher } from "@/utils/frontendLogBatcher";
import { consoleMessage, installConsoleLogCapture } from "@/utils/consoleLogCapture";
import { isBenignTauriCallbackWarning } from "@/shared/platform/tauriWebview";
import {
  FRONTEND_LOG_BATCH_MAX_DELAY_MS,
  FRONTEND_LOG_BATCH_MAX_ENTRIES,
  FRONTEND_LOG_BATCH_MAX_PENDING,
  FRONTEND_LOG_MESSAGE_MAX_BYTES,
  isFrontendLogEnabled,
} from "@/utils/logConfig";

export const frontendLogBatcher = createFrontendLogBatcher({
  maxBatchEntries: FRONTEND_LOG_BATCH_MAX_ENTRIES,
  maxPendingEntries: FRONTEND_LOG_BATCH_MAX_PENDING,
  maxDelayMs: FRONTEND_LOG_BATCH_MAX_DELAY_MS,
  maxMessageBytes: FRONTEND_LOG_MESSAGE_MAX_BYTES,
  submit: (entries) => LogService.submitFrontendLogs(entries),
});

let disposeCapture: (() => void) | undefined;

export function installFrontendLogging(): () => void {
  if (disposeCapture) return disposeCapture;
  const stopCapture = installConsoleLogCapture((level, args) => {
    if (!isFrontendLogEnabled(level)) return;
    if (import.meta.hot && isBenignTauriCallbackWarning(args)) return;
    frontendLogBatcher.enqueue({
      level,
      domain: "ui",
      target: "frontend.console",
      message: consoleMessage(args, FRONTEND_LOG_MESSAGE_MAX_BYTES),
      fields: {},
    });
  });
  disposeCapture = () => {
    stopCapture();
    disposeCapture = undefined;
  };
  return disposeCapture;
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    disposeCapture?.();
    disposeCapture = undefined;
    frontendLogBatcher.dispose();
  });
}
