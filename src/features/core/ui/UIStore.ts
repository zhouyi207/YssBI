import {
  DialogOptions,
  InputDialogOptions,
  MessageDialogOptions,
  ConfirmTriResult,
  ProgressState,
} from "@/shared/types/ui";
import type {
  ApplicationUiState,
  ExcelSheetSelectDialogOptions,
  ImportDialogOptions,
  SqlConnectionDialogOptions,
  SqliteTableSelectDialogOptions,
  SqlRemoteTableSelectDialogOptions,
} from "./applicationUiTypes";

type Listener = () => void;

class UIStore {
  private state: ApplicationUiState = {
    modals: [],
    progress: null,
  };

  private listeners = new Set<Listener>();
  private progressOnCancel: (() => void) | null = null;

  // --- subscription ---
  subscribe(listener: Listener) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private emit() {
    this.listeners.forEach((l) => l());
  }

  getState(): ApplicationUiState {
    return this.state;
  }

  // --- Modal Stack ---
  showSettings() {
    if (this.state.modals.some((modal) => modal.type === "settings")) return;
    this.state = {
      ...this.state,
      modals: [...this.state.modals, { id: crypto.randomUUID(), type: "settings" }],
    };
    this.emit();
  }

  private showDialog(options: DialogOptions, parentId?: string) {
    if (parentId && !this.state.modals.some((modal) => modal.id === parentId)) {
      options.onCancel?.();
      return;
    }
    this.state = {
      ...this.state,
      modals: [
        ...this.state.modals,
        {
          id: crypto.randomUUID(),
          type: "confirm",
          parentId,
          options,
        },
      ],
    };
    this.emit();
  }

  alert(options: MessageDialogOptions): Promise<void> {
    return new Promise((resolve) => {
      const id = crypto.randomUUID();
      this.state = {
        ...this.state,
        modals: [...this.state.modals, { id, type: "message", options }],
      };
      this.emit();
      const unsubscribe = this.subscribe(() => {
        if (this.state.modals.some((modal) => modal.id === id)) return;
        unsubscribe();
        resolve();
      });
    });
  }

  confirm(
    options: Omit<DialogOptions, "onConfirm" | "onCancel">,
    parentId?: string,
  ): Promise<boolean> {
    return new Promise((resolve) => {
      this.showDialog(
        {
          ...options,
          onConfirm: () => resolve(true),
          onCancel: () => resolve(false),
        },
        parentId,
      );
    });
  }

  /**
   * Tri-state confirm with three actions: confirm / discard / cancel.
   * Use for destructive flows where the user can either commit, abandon, or back out.
   */
  confirm3(
    options: Omit<DialogOptions, "onConfirm" | "onCancel" | "onDiscard"> & { discardText: string },
  ): Promise<ConfirmTriResult> {
    return new Promise((resolve) => {
      this.showDialog({
        ...options,
        onConfirm: () => resolve("confirm"),
        onDiscard: () => resolve("discard"),
        onCancel: () => resolve("cancel"),
      });
    });
  }

  prompt(options: Omit<InputDialogOptions, "onSubmit" | "onCancel">): Promise<string | null> {
    return new Promise((resolve) => {
      this.state = {
        ...this.state,
        modals: [
          ...this.state.modals,
          {
            id: crypto.randomUUID(),
            type: "input",
            options: {
              ...options,
              onSubmit: (value) => resolve(value),
              onCancel: () => resolve(null),
            },
          },
        ],
      };
      this.emit();
    });
  }

  showImportDialog(options: ImportDialogOptions) {
    const existing = this.state.modals.find((modal) => modal.type === "import");
    if (existing) return existing.id;
    const id = crypto.randomUUID();
    this.state = {
      ...this.state,
      modals: [
        ...this.state.modals,
        {
          id,
          type: "import",
          options,
        },
      ],
    };
    this.emit();
    return id;
  }

  showSqliteTableSelectDialog(options: SqliteTableSelectDialogOptions, parentId: string) {
    if (!this.state.modals.some((modal) => modal.id === parentId)) return;
    this.state = {
      ...this.state,
      modals: [
        ...this.state.modals,
        {
          id: crypto.randomUUID(),
          type: "sqliteTableSelect",
          parentId,
          options,
        },
      ],
    };
    this.emit();
  }

  showExcelSheetSelectDialog(options: ExcelSheetSelectDialogOptions, parentId: string) {
    if (!this.state.modals.some((modal) => modal.id === parentId)) return;
    this.state = {
      ...this.state,
      modals: [
        ...this.state.modals,
        {
          id: crypto.randomUUID(),
          type: "excelSheetSelect",
          parentId,
          options,
        },
      ],
    };
    this.emit();
  }

  showSqlConnectionDialog(options: SqlConnectionDialogOptions, parentId: string) {
    if (!this.state.modals.some((modal) => modal.id === parentId)) return;
    const id = crypto.randomUUID();
    this.state = {
      ...this.state,
      modals: [
        ...this.state.modals,
        {
          id,
          type: "sqlConnection",
          parentId,
          options,
        },
      ],
    };
    this.emit();
    return id;
  }

  showSqlRemoteTableSelectDialog(options: SqlRemoteTableSelectDialogOptions, parentId: string) {
    if (!this.state.modals.some((modal) => modal.id === parentId)) return;
    this.state = {
      ...this.state,
      modals: [
        ...this.state.modals,
        {
          id: crypto.randomUUID(),
          type: "sqlRemoteTableSelect",
          parentId,
          options,
        },
      ],
    };
    this.emit();
  }

  closeModal(id: string) {
    if (!this.state.modals.some((modal) => modal.id === id)) return;
    const removedIds = new Set([id]);
    // Children are appended after their parent; closing a workflow removes only its descendants.
    const newModals = this.state.modals.filter((modal) => {
      if (modal.parentId && removedIds.has(modal.parentId)) removedIds.add(modal.id);
      return !removedIds.has(modal.id);
    });
    const removedModals = this.state.modals.filter((modal) => removedIds.has(modal.id));
    this.state = {
      ...this.state,
      modals: newModals,
    };
    this.emit();
    // Complete programmatically dismissed prompts too. A user's decision has already settled its promise.
    for (const modal of removedModals) {
      if (modal.type === "confirm" || modal.type === "input") modal.options.onCancel?.();
    }
  }

  // --- Progress Overlay ---
  /** 启动全局进度蒙层；同一时刻只有一个进度任务。 */
  startProgress(progress: ProgressState, options?: { onCancel?: () => void }) {
    this.progressOnCancel = options?.onCancel ?? null;
    this.state = {
      ...this.state,
      progress: {
        ...progress,
        cancelable: progress.cancelable ?? !!options?.onCancel,
      },
    };
    this.emit();
  }

  /**
   * 更新当前进度。若当前没有进度任务则忽略，避免在已 finishProgress 之后
   * 因异步竞态而误恢复出蒙层。
   */
  updateProgress(patch: Partial<ProgressState>) {
    if (!this.state.progress) return;
    this.state = {
      ...this.state,
      progress: { ...this.state.progress, ...patch },
    };
    this.emit();
  }

  /** 用户点击进度蒙层关闭按钮时调用。 */
  cancelProgress() {
    if (!this.state.progress?.cancelable) return;
    this.progressOnCancel?.();
    this.finishProgress();
  }

  /** 关闭全局进度蒙层。多次调用是幂等的。 */
  finishProgress() {
    if (!this.state.progress) return;
    this.progressOnCancel = null;
    this.state = { ...this.state, progress: null };
    this.emit();
  }
}

export const uiStore = new UIStore();
