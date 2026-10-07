import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  submit: vi.fn(),
}));

vi.mock("@/services/log", () => ({
  LogService: { submitFrontendLogs: mocks.submit },
}));

import { FRONTEND_LOG_BATCH_MAX_DELAY_MS } from "./logConfig";
import { logger } from "@/utils/frontendLogger";
import { installFrontendLogging } from "@/features/application/observability/frontendLogTransport";
import { useResourceStore } from "@/features/core/resource/resourceStore";

describe("frontend logging", () => {
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

  it("preserves a Core diagnostic's domain and source through the shared transport", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    stopCapture = installFrontendLogging();
    useResourceStore.getState().updateDatabaseMetadata("missing-log-fixture", 1, {
      columns: [],
      rowCount: 0,
      columnCount: 0,
    });
    await vi.advanceTimersByTimeAsync(FRONTEND_LOG_BATCH_MAX_DELAY_MS);
    expect(mocks.submit).toHaveBeenCalledOnce();
    expect(mocks.submit.mock.calls[0]?.[0]).toEqual([
      {
        level: "warn",
        domain: "data",
        target: "ResourceStore",
        message: 'updateDatabaseMetadata: id "missing-log-fixture" not found',
        source: "ResourceStore",
        fields: {},
      },
    ]);
  });

  it("summarizes captured Error objects without reading their private text", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    stopCapture = installFrontendLogging();
    const readPrivate = vi.fn(() => "private command content");
    const failure = new Error();
    Object.defineProperties(failure, {
      name: { get: readPrivate },
      message: { get: readPrivate },
    });
    console.error("Command failed", failure);
    await vi.advanceTimersByTimeAsync(FRONTEND_LOG_BATCH_MAX_DELAY_MS);
    expect(readPrivate).not.toHaveBeenCalled();
    expect(mocks.submit).toHaveBeenCalledExactlyOnceWith([
      expect.objectContaining({
        level: "error",
        target: "frontend.console",
        message: "Command failed [Error]",
      }),
    ]);
  });

  it("disposes pending transport work and keeps a replacement installation after late cleanup", async () => {
    vi.spyOn(console, "log").mockImplementation(() => {});
    const first = installFrontendLogging();
    logger.app.info("discarded before dispatch");
    first();
    stopCapture = installFrontendLogging();
    first();
    expect.soft(installFrontendLogging()).toBe(stopCapture);
    logger.app.info("current installation");
    await vi.advanceTimersByTimeAsync(FRONTEND_LOG_BATCH_MAX_DELAY_MS);
    expect(mocks.submit).toHaveBeenCalledOnce();
    expect(mocks.submit.mock.calls[0]?.[0]).toMatchObject([{ message: "current installation" }]);
    expect(mocks.submit.mock.calls[0]?.[0]).toHaveLength(1);
    stopCapture();
    logger.app.info("console only after cleanup");
    await vi.advanceTimersByTimeAsync(FRONTEND_LOG_BATCH_MAX_DELAY_MS);
    expect(mocks.submit).toHaveBeenCalledOnce();
  });
});
