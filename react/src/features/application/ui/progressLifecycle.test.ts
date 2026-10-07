// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { uiStore } from "@/features/core/ui/UIStore";
import { runWithDataOperationProgress } from "../dataManagement/dataOperationProgress";
import {
  runWithProjectPickerProgress,
  type ProjectPickerProgressHandle,
} from "../project/projectPickerProgress";
import { applicationUi } from "./applicationUi";

function pendingTask() {
  let complete!: () => void;
  const promise = new Promise<void>((resolve) => {
    complete = resolve;
  });
  return { promise, complete };
}

beforeEach(() => {
  vi.spyOn(globalThis, "requestAnimationFrame").mockImplementation((callback) => {
    queueMicrotask(() => callback(0));
    return 0;
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  expect(uiStore.getState().progress).toBeNull();
});

it("does not publish unchanged progress patches", () => {
  const progress = uiStore.startProgress({ stage: "scanning", percent: 0.5 });
  const before = uiStore.getState().progress;
  const notified = vi.fn();
  const unsubscribe = uiStore.subscribe(notified);
  try {
    progress.update({ stage: "scanning", percent: 0.5 });
    expect(uiStore.getState().progress).toBe(before);
    expect(notified).not.toHaveBeenCalled();
    progress.update({ percent: 0.75 });
    expect(uiStore.getState().progress).toEqual({ ...before, percent: 0.75 });
    expect(notified).toHaveBeenCalledOnce();
  } finally {
    unsubscribe();
    progress.finish();
  }
});

it("keeps a newer import progress when an earlier project task updates and finishes", async () => {
  const projectTask = pendingTask();
  const importTask = pendingTask();
  let projectProgress!: ProjectPickerProgressHandle;
  const project = runWithProjectPickerProgress(
    { initial: { stage: "old project" } },
    (progress) => {
      projectProgress = progress;
      return projectTask.promise;
    },
  );
  const importing = runWithDataOperationProgress("new import", undefined, () => importTask.promise);
  try {
    const current = uiStore.getState().progress;
    expect(current?.stage).toBe("new import");
    projectProgress.update({ stage: "late project event", percent: 1 });
    expect(uiStore.getState().progress).toBe(current);
    projectTask.complete();
    await project;
    expect(uiStore.getState().progress).toBe(current);
    importTask.complete();
    await importing;
    expect(uiStore.getState().progress).toBeNull();
  } finally {
    projectTask.complete();
    importTask.complete();
    await Promise.all([project, importing]);
  }
});

it("cancels the captured task without clearing a replacement started by its cancel callback", async () => {
  const projectTask = pendingTask();
  const importTask = pendingTask();
  let importing: Promise<void> | undefined;
  const project = runWithProjectPickerProgress(
    {
      initial: { stage: "project scan", cancelable: true },
      onCancel: () => {
        importing = runWithDataOperationProgress(
          "replacement import",
          undefined,
          () => importTask.promise,
        );
      },
    },
    () => projectTask.promise,
  );
  try {
    applicationUi.cancelProgress();
    expect(importing).toBeDefined();
    const current = uiStore.getState().progress;
    expect(current?.stage).toBe("replacement import");
    projectTask.complete();
    expect((await project).cancelled).toBe(true);
    expect(uiStore.getState().progress).toBe(current);
    importTask.complete();
    await importing;
    expect(uiStore.getState().progress).toBeNull();
  } finally {
    projectTask.complete();
    importTask.complete();
    await Promise.all([project, importing]);
  }
});
