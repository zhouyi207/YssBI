import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  submit: vi.fn(),
}));

vi.mock("@/services/log", () => ({
  LogService: { submitFrontendLogs: mocks.submit },
}));

import { FRONTEND_LOG_BATCH_MAX_DELAY_MS } from "@/shared/config-default";
import { logger } from "@/features/application/observability/appLogger";
import { installFrontendLogging } from "@/features/application/observability/frontendLogTransport";

describe("appLogger", () => {
  let stopCapture: (() => void) | undefined;
  beforeEach(() => {
    vi.useFakeTimers();
    mocks.submit.mockReset().mockResolvedValue(undefined);
  });

  afterEach(() => {
    stopCapture?.();
    stopCapture = undefined;
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it("filters debug and trace before console capture formats messages or enqueues batches", async () => {
    const consoleDebug = vi.spyOn(console, "debug").mockImplementation(() => {});
    vi.spyOn(console, "trace").mockImplementation(() => {});
    stopCapture = installFrontendLogging();

    logger.app.debug("disabled application log");
    logger.exec.trace("disabled execution log");
    expect(consoleDebug).not.toHaveBeenCalled();

    const message = new Error("disabled");
    const readMessage = vi.fn(() => "disabled");
    Object.defineProperty(message, "message", { get: readMessage });
    console.debug(message);
    console.trace(message);
    await vi.advanceTimersByTimeAsync(FRONTEND_LOG_BATCH_MAX_DELAY_MS);

    expect(readMessage).not.toHaveBeenCalled();
    expect(mocks.submit).not.toHaveBeenCalled();
  });

  it("captures console and explicit logs once without feeding transport failures back into the logger", async () => {
    const consoleLog = vi.spyOn(console, "log").mockImplementation(() => {});
    stopCapture = installFrontendLogging();

    logger.app.info("application ready", "Bootstrap");
    expect(consoleLog).toHaveBeenCalledOnce();
    expect(consoleLog).toHaveBeenCalledWith("[APP][Bootstrap] application ready");

    await vi.advanceTimersByTimeAsync(FRONTEND_LOG_BATCH_MAX_DELAY_MS);
    expect(mocks.submit).toHaveBeenCalledOnce();
    expect(mocks.submit.mock.calls[0]?.[0]).toMatchObject([
      {
        level: "info",
        domain: "application",
        target: "Bootstrap",
        message: "application ready",
        source: "Bootstrap",
        fields: {},
      },
    ]);

    console.log("raw console message");
    await vi.advanceTimersByTimeAsync(FRONTEND_LOG_BATCH_MAX_DELAY_MS);
    expect(mocks.submit).toHaveBeenCalledTimes(2);
    expect(mocks.submit.mock.calls[1]?.[0]).toMatchObject([
      { level: "info", target: "frontend.console", message: "raw console message" },
    ]);
    mocks.submit.mockRejectedValue(new Error("transport unavailable"));
    console.log("last message");
    await vi.advanceTimersByTimeAsync(FRONTEND_LOG_BATCH_MAX_DELAY_MS * 10);
    expect(mocks.submit).toHaveBeenCalledTimes(3);
  });
});
