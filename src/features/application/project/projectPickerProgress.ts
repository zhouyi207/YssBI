import { i18n } from "@/app/i18n";
import { uiStore } from "@/features/core/ui/UIStore";
import type { ProgressState } from "@/shared/types/ui";
import type {
  ProjectCleanupProgressEvent,
  ProjectScanProgressEvent,
} from "@/services/project/projectService";

export interface ProjectPickerProgressHandle {
  update: (patch: Partial<ProgressState>) => void;
  markDone: () => void;
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

export type ProjectPickerProgressTask = "scan" | "cleanup" | "create" | "open";

export function projectPickerProgressInitial(
  task: ProjectPickerProgressTask,
): RunProjectPickerProgressOptions["initial"] {
  switch (task) {
    case "scan":
      return {
        stage: i18n.t("projectPicker.loading.scanning"),
        detail: i18n.t("projectPicker.loading.scanningFolder"),
        cancelable: true,
      };
    case "cleanup":
      return {
        stage: i18n.t("projectPicker.loading.cleanup"),
        cancelable: true,
      };
    case "create":
      return {
        stage: i18n.t("projectPicker.loading.creating"),
        percent: 0.2,
      };
    case "open":
      return {
        stage: i18n.t("projectPicker.loading.opening"),
        detail: i18n.t("projectPicker.loading.readingFile"),
        percent: 0.1,
      };
  }
}

export function projectPickerScanFolderTitle(): string {
  return i18n.t("projectPicker.scanFolderTitle");
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
  update: ProjectPickerProgressHandle["update"],
  stage: OpenProjectStage,
) {
  update({
    detail: i18n.t(`projectPicker.loading.${stage}`),
    percent: OPEN_PROJECT_STAGE_PERCENT[stage],
  });
}

export function applyScanProgressEvent(
  event: ProjectScanProgressEvent,
  update: ProjectPickerProgressHandle["update"],
) {
  if (event.kind === "scanning") {
    update({
      stage: i18n.t("projectPicker.loading.scanning"),
      detail: i18n.t("projectPicker.loading.scanningFolder"),
    });
    return;
  }
  if (event.kind === "discovered") {
    update({
      detail: i18n.t("projectPicker.loading.scanFoundProjects", { count: event.count }),
      percent: event.count > 0 ? 0.5 : 0.9,
    });
    return;
  }
  if (event.kind === "registering") {
    const fraction = event.total > 0 ? event.current / event.total : 1;
    update({
      detail: i18n.t("projectPicker.loading.scanRegistering", {
        current: event.current,
        total: event.total,
      }),
      percent: 0.5 + 0.45 * fraction,
    });
  }
}

export function applyCleanupProgressEvent(
  event: ProjectCleanupProgressEvent,
  update: ProjectPickerProgressHandle["update"],
) {
  if (event.kind === "checking") {
    const fraction = event.total > 0 ? event.current / event.total : 1;
    update({
      stage: i18n.t("projectPicker.loading.cleanup"),
      detail: i18n.t("projectPicker.loading.cleanupChecking", {
        current: event.current,
        total: event.total,
      }),
      percent: 0.1 + 0.75 * fraction,
    });
    return;
  }
  if (event.kind === "removing") {
    update({
      detail: i18n.t("projectPicker.loading.cleanupRemoving", { removed: event.removed }),
      percent: 0.85 + (0.1 * Math.min(event.removed, event.total)) / Math.max(event.total, 1),
    });
  }
}

export function markProjectPickerProgressDone(update: ProjectPickerProgressHandle["update"]) {
  update({
    detail: i18n.t("projectPicker.loading.done"),
    percent: 1,
  });
}
