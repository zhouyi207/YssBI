import { resultReferenceFixture } from "@/tests/helpers/resultFixture";
import { resultLeases } from "@/features/application/results/resultLeases";
vi.mock("@/features/application/results/resultLeases", () => ({
  resultLeases: {
    acquire: vi.fn(async () => ({ leaseId: "00000000-0000-0000-0000-000000000100" })),
    finish: vi.fn(async () => {}),
  },
}));
import { beforeEach, describe, expect, it, vi } from "vitest";
import { IPC_TRANSPORT_FAILURE_CODE, normalizeIpcError } from "@/services/ipc";
import { openDatabaseEditorWindow } from "./openDatabaseEditor";
import { openLogsWindow } from "./openLogsWindow";
import { openPresentationWindow } from "./openPresentationWindow";

const createPersistedWindow = vi.hoisted(() => vi.fn());
const appError = vi.hoisted(() => vi.fn());
const execError = vi.hoisted(() => vi.fn());

vi.mock("./createPersistedWindow", () => ({ createPersistedWindow }));
vi.mock("./windowLabels", () => ({ createEphemeralWindowLabel: (kind: string) => `${kind}-test` }));
vi.mock("@/features/application/observability/appLogger", () => ({
  logger: { app: { error: appError }, exec: { error: execError } },
}));
vi.mock("@/app/i18n", () => ({ i18n: { t: (key: string) => `localized:${key}` } }));

const helpers = [
  ["database editor", () => openDatabaseEditorWindow("database-1")],
  ["logs", () => openLogsWindow()],
] as const;

describe("window opening helpers", () => {
  beforeEach(() => vi.clearAllMocks());

  it.each(helpers)("records and rethrows a %s window failure", async (_label, openWindow) => {
    const failure = new Error("sensitive native window failure");
    createPersistedWindow.mockRejectedValueOnce(failure);

    await expect(openWindow()).rejects.toBe(failure);

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
        route: "/plot",
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
