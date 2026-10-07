import { resultReferenceFixture } from "@/tests/helpers/resultFixture";
import { resultLeases } from "@/features/application/results/resultLeases";
const acquire = vi.hoisted(() =>
  vi.fn(async () => ({ leaseId: "00000000-0000-0000-0000-000000000100" })),
);
vi.mock("@/features/application/results/resultLeases", () => ({
  resultLeases: {
    acquire,
    finish: vi.fn(async () => {}),
  },
}));
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { IPC_TRANSPORT_FAILURE_CODE, normalizeIpcError } from "@/services/ipc";
import { openLogsWindow } from "./openLogsWindow";
import { openPresentationWindow } from "./openPresentationWindow";

const createPersistedWindow = vi.hoisted(() => vi.fn());
const appError = vi.hoisted(() => vi.fn());
const execError = vi.hoisted(() => vi.fn());

vi.mock("./createPersistedWindow", () => ({ createPersistedWindow }));
vi.mock("./windowLabels", () => ({ createEphemeralWindowLabel: (kind: string) => `${kind}-test` }));
vi.mock("@/utils/frontendLogger", () => ({
  logger: { app: { error: appError }, exec: { error: execError } },
}));
vi.mock("@/app/i18n", () => ({ i18n: { t: (key: string) => `localized:${key}` } }));

describe("window opening helpers", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearProjectLifecycle();
  });
  afterEach(() => clearProjectLifecycle());

  it("records and rethrows a logs window failure", async () => {
    const failure = new Error("sensitive native window failure");
    createPersistedWindow.mockRejectedValueOnce(failure);

    await expect(openLogsWindow()).rejects.toBe(failure);

    expect(appError).toHaveBeenCalledOnce();
    expect(String(appError.mock.calls[0]?.[0])).toContain(IPC_TRANSPORT_FAILURE_CODE);
    expect(JSON.stringify(appError.mock.calls)).not.toContain("sensitive native window failure");
  });

  it("opens Logs with its shared native window kind", async () => {
    createPersistedWindow.mockResolvedValueOnce(undefined);

    await openLogsWindow();

    expect(createPersistedWindow).toHaveBeenCalledWith(
      expect.objectContaining({
        kind: "logs",
        label: "logs-test",
        url: "index.html#/logs",
      }),
    );
  });

  it("records and rethrows a presentation window failure", async () => {
    const failure = new Error("sensitive presentation window failure");
    createPersistedWindow.mockRejectedValueOnce(failure);

    await expect(
      openPresentationWindow(resultReferenceFixture("result-1"), {
        kind: "plot",
        windowTitle: "Plot",
      }),
    ).rejects.toBe(failure);

    expect(execError).toHaveBeenCalledWith(
      expect.stringContaining(`code=${IPC_TRANSPORT_FAILURE_CODE}`),
      "Window",
    );
    expect(JSON.stringify(execError.mock.calls)).not.toContain(
      "sensitive presentation window failure",
    );
    expect(resultLeases.finish).toHaveBeenCalledWith("00000000-0000-0000-0000-000000000100", false);
  });

  it("releases a late handoff instead of opening after project lifecycle replacement", async () => {
    const reference = resultReferenceFixture("17");
    const presentation = { kind: "plot", windowTitle: "Plot" } as const;
    createPersistedWindow.mockResolvedValue(undefined);
    await openPresentationWindow(reference, presentation);
    expect(createPersistedWindow).toHaveBeenCalledOnce();
    expect(resultLeases.finish).toHaveBeenCalledWith("00000000-0000-0000-0000-000000000100", true);

    for (const previous of [null, "project-a"]) {
      vi.clearAllMocks();
      clearProjectLifecycle();
      if (previous) startProjectLifecycle(previous);
      let resolve!: (value: { leaseId: string }) => void;
      acquire.mockReturnValueOnce(
        new Promise((settle) => {
          resolve = settle;
        }),
      );
      const opening = openPresentationWindow(reference, presentation);
      startProjectLifecycle("project-a");
      resolve({ leaseId: "00000000-0000-0000-0000-000000000100" });
      await opening;
      expect.soft(createPersistedWindow, String(previous)).not.toHaveBeenCalled();
      expect
        .soft(resultLeases.finish, String(previous))
        .toHaveBeenCalledExactlyOnceWith("00000000-0000-0000-0000-000000000100", false);
      expect.soft(execError, String(previous)).not.toHaveBeenCalled();
    }
  });

  it("records a backend incident ID before rethrowing the original IPC error", async () => {
    const failure = normalizeIpcError("create_window", {
      code: "window_creation_failed",
      details: null,
      incidentId: "incident-window-42",
    });
    createPersistedWindow.mockRejectedValueOnce(failure);

    await expect(openLogsWindow()).rejects.toBe(failure);

    expect(appError).toHaveBeenCalledWith(
      expect.stringContaining("code=window_creation_failed incidentId=incident-window-42"),
      "Window",
    );
  });
});
