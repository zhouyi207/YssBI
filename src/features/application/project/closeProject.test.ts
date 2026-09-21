import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
  captureProjectLifecycleState,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { applyProjectClosed, requestCloseProject } from "./closeProject";

const mocks = vi.hoisted(() => ({
  confirm: vi.fn(),
  flush: vi.fn(),
  close: vi.fn(),
  clear: vi.fn(),
  resetResults: vi.fn(),
  error: vi.fn(),
  store: { projectInstanceId: "project-a" as string | null, currentPath: "A" as string | null },
}));
vi.mock("@/app/i18n", () => ({ i18n: { t: (key: string) => key } }));
vi.mock("@/services/project/projectService", () => ({
  ProjectService: { closeProject: mocks.close },
}));
vi.mock("@/features/application/editor/confirmDirtyEditorClose", () => ({
  confirmDirtyEditorClose: mocks.confirm,
}));
vi.mock("@/features/application/editor/blockingErrorDialog", () => ({
  showBlockingIpcError: mocks.error,
}));
vi.mock("@/modules/workbench/public", () => ({
  workbenchLayoutController: { flushBeforeWindowClose: mocks.flush },
}));
vi.mock("./projectIOStore", () => ({ useProjectIOStore: { getState: () => mocks.store } }));
vi.mock("@/features/application/projectLifecycleReceiptDependencies", () => ({
  createProjectLifecycleReceiptDependencies: () => ({ clearProject: mocks.clear }),
}));
vi.mock("@/features/application/results", () => ({ resetResultQueryProject: mocks.resetResults }));
vi.mock("@/features/application/editorMutation/projectPublicationCoordinator", () => ({
  projectPublicationCoordinator: { cancelProject: () => clearProjectLifecycle() },
}));

beforeEach(() => {
  vi.resetAllMocks();
  startProjectLifecycle("project-a");
  mocks.store = { projectInstanceId: "project-a", currentPath: "A" };
  mocks.confirm.mockResolvedValue(true);
  mocks.flush.mockResolvedValue(undefined);
  mocks.clear.mockImplementation(async () => {
    mocks.store = { projectInstanceId: null, currentPath: null };
  });
});

describe("close project", () => {
  it("preserves the project when confirmation is cancelled or the backend rejects close", async () => {
    mocks.confirm.mockResolvedValueOnce(false);
    await expect(requestCloseProject()).resolves.toBe(false);
    expect(mocks.close).not.toHaveBeenCalled();
    mocks.close.mockRejectedValueOnce(new Error("close rejected"));
    await expect(requestCloseProject()).resolves.toBe(false);
    expect(mocks.clear).not.toHaveBeenCalled();
    expect(mocks.resetResults).not.toHaveBeenCalled();
    expect(mocks.store.projectInstanceId).toBe("project-a");
    expect(captureProjectLifecycleState().projectInstanceId).toBe("project-a");
  });

  it("waits for shared event/direct cleanup and ignores an old close after another project opens", async () => {
    let finishClear!: () => void;
    mocks.clear.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finishClear = () => {
            mocks.store = { projectInstanceId: null, currentPath: null };
            resolve();
          };
        }),
    );
    mocks.close.mockImplementation(async (id: string) => {
      void applyProjectClosed(id);
    });
    const first = requestCloseProject();
    expect(requestCloseProject()).toBe(first);
    let completed = false;
    void first.then(() => {
      completed = true;
    });
    await vi.waitFor(() => expect(mocks.clear).toHaveBeenCalledOnce());
    expect(completed).toBe(false);
    finishClear();
    await expect(first).resolves.toBe(true);
    expect(mocks.close).toHaveBeenCalledExactlyOnceWith("project-a");
    expect(mocks.resetResults).toHaveBeenCalledOnce();
    startProjectLifecycle("project-b");
    mocks.store = { projectInstanceId: "project-b", currentPath: "B" };
    await applyProjectClosed("project-a");
    expect(mocks.clear).toHaveBeenCalledOnce();
    expect(mocks.store.projectInstanceId).toBe("project-b");
    expect(captureProjectLifecycleState().projectInstanceId).toBe("project-b");
  });
});
