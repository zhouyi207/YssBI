import type { TFunction } from "i18next";
import { uiStore } from "@/features/core/ui/UIStore";
import type { ProgressState } from "@/shared/types/ui";
import type {
  ProjectCleanupProgressEvent,
  ProjectScanProgressEvent,
} from "@/services/project/projectService";

export interface ProjectPickerProgressHandle {
  update: (patch: Partial<ProgressState>) => void;
  isCancelled: () => boolean;
}

export interface RunProjectPickerProgressOptions {
  initial: Pick<ProgressState, "stage" | "detail" | "percent" | "cancelable">;
  onCancel?: () => void;
}

export interface ProjectPickerProgressRun<T> {
  result: T;
  cancelled: boolean;
}

/** 统一项目选择页进度蒙层生命周期（start → update → finish）。 */
export async function runWithProjectPickerProgress<T>(
  options: RunProjectPickerProgressOptions,
  run: (progress: ProjectPickerProgressHandle) => Promise<T>,
): Promise<ProjectPickerProgressRun<T>> {
  let cancelled = false;

  uiStore.startProgress(
    options.initial,
    options.onCancel
      ? {
          onCancel: () => {
            cancelled = true;
            options.onCancel?.();
          },
        }
      : undefined,
  );

  try {
    const result = await run({
      update: (patch) => {
        if (!cancelled) {
          uiStore.updateProgress(patch);
        }
      },
      markDone: () => {
        if (!cancelled) {
          uiStore.updateProgress({
            detail: undefined,
            percent: 1,
          });
        }
      },
      isCancelled: () => cancelled,
    });
    return { result, cancelled };
  } finally {
    uiStore.finishProgress();
  }
}

const OPEN_PROJECT_STAGE_PERCENT = {
  readingFile: 0.1,
  loadingData: 0.5,
  preparingEditor: 0.9,
  done: 1,
} as const;

type OpenProjectStage = keyof typeof OPEN_PROJECT_STAGE_PERCENT;

export function updateOpenProjectProgressStage(
  t: TFunction,
  update: ProjectPickerProgressHandle["update"],
  stage: OpenProjectStage,
) {
  update({
    detail: t(`projectPicker.loading.${stage}`),
    percent: OPEN_PROJECT_STAGE_PERCENT[stage],
  });
}

export function applyScanProgressEvent(
  t: TFunction,
  event: ProjectScanProgressEvent,
  update: ProjectPickerProgressHandle["update"],
) {
  if (event.kind === "scanning") {
    update({
      stage: t("projectPicker.loading.scanning"),
      detail: t("projectPicker.loading.scanningFolder"),
    });
    return;
  }
  if (event.kind === "discovered") {
    update({
      detail: t("projectPicker.loading.scanFoundProjects", { count: event.count }),
      percent: event.count > 0 ? 0.5 : 0.9,
    });
    return;
  }
  if (event.kind === "registering") {
    const fraction = event.total > 0 ? event.current / event.total : 1;
    update({
      detail: t("projectPicker.loading.scanRegistering", {
        current: event.current,
        total: event.total,
      }),
      percent: 0.5 + 0.45 * fraction,
    });
  }
}

export function applyCleanupProgressEvent(
  t: TFunction,
  event: ProjectCleanupProgressEvent,
  update: ProjectPickerProgressHandle["update"],
) {
  if (event.kind === "checking") {
    const fraction = event.total > 0 ? event.current / event.total : 1;
    update({
      stage: t("projectPicker.loading.cleanup"),
      detail: t("projectPicker.loading.cleanupChecking", {
        current: event.current,
        total: event.total,
      }),
      percent: 0.1 + 0.75 * fraction,
    });
    return;
  }
  if (event.kind === "removing") {
    update({
      detail: t("projectPicker.loading.cleanupRemoving", { removed: event.removed }),
      percent: 0.85 + 0.1 * Math.min(event.removed, event.total) / Math.max(event.total, 1),
    });
  }
}

export function markProjectPickerProgressDone(t: TFunction, update: ProjectPickerProgressHandle["update"]) {
  update({
    detail: t("projectPicker.loading.done"),
    percent: 1,
  });
}
