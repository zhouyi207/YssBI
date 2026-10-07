import { LogService } from "@/services/log";
import { createFrontendLogBatcher } from "@/utils/frontendLogBatcher";
import { bindFrontendLogSink } from "@/utils/frontendLogger";
import { consoleMessage, installConsoleLogCapture } from "@/utils/consoleLogCapture";
import { isBenignTauriCallbackWarning } from "@/shared/platform/tauriWebview";
import {
  FRONTEND_LOG_BATCH_MAX_DELAY_MS,
  FRONTEND_LOG_BATCH_MAX_ENTRIES,
  FRONTEND_LOG_BATCH_MAX_PENDING,
  FRONTEND_LOG_MESSAGE_MAX_BYTES,
  isFrontendLogEnabled,
} from "@/utils/logConfig";

let disposeLogging: (() => void) | undefined;

export function installFrontendLogging(): () => void {
  if (disposeLogging) return disposeLogging;
  const batcher = createFrontendLogBatcher({
    maxBatchEntries: FRONTEND_LOG_BATCH_MAX_ENTRIES,
    maxPendingEntries: FRONTEND_LOG_BATCH_MAX_PENDING,
    maxDelayMs: FRONTEND_LOG_BATCH_MAX_DELAY_MS,
    maxMessageBytes: FRONTEND_LOG_MESSAGE_MAX_BYTES,
    submit: (entries) => LogService.submitFrontendLogs(entries),
  });
  const stopLogger = bindFrontendLogSink(batcher.enqueue);
  const stopCapture = installConsoleLogCapture((level, args) => {
    if (!isFrontendLogEnabled(level)) return;
    if (import.meta.hot && isBenignTauriCallbackWarning(args)) return;
    batcher.enqueue({
      level,
      domain: "ui",
      target: "frontend.console",
      message: consoleMessage(args, FRONTEND_LOG_MESSAGE_MAX_BYTES),
      fields: {},
    });
  });
  const dispose = () => {
    stopLogger();
    stopCapture();
    batcher.dispose();
    if (disposeLogging === dispose) disposeLogging = undefined;
  };
  disposeLogging = dispose;
  return dispose;
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    disposeLogging?.();
  });
}
