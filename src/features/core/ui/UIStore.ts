import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import {
    Message,
    MessageType,
    DialogOptions,
    InputDialogOptions,
    ImportDialogOptions,
    SqliteTableSelectDialogOptions,
    ExcelSheetSelectDialogOptions,
    SqlConnectionDialogOptions,
    SqlRemoteTableSelectDialogOptions,
    ConfirmTriResult,
    ProgressState,
} from "@/shared/types/ui";

type UIModal =
    | { id: string; type: "confirm"; options: DialogOptions }
    | { id: string; type: "input"; options: InputDialogOptions }
    | { id: string; type: "import"; options: ImportDialogOptions }
    | { id: string; type: "sqliteTableSelect"; options: SqliteTableSelectDialogOptions }
    | { id: string; type: "excelSheetSelect"; options: ExcelSheetSelectDialogOptions }
    | { id: string; type: "sqlConnection"; options: SqlConnectionDialogOptions }
    | { id: string; type: "sqlRemoteTableSelect"; options: SqlRemoteTableSelectDialogOptions };

type UIState = {
  messages: Message[];
  modals: UIModal[];
  /** 全局进度蒙层；为 null 时不显示。 */
  progress: ProgressState | null;
};

type Listener = () => void;

class UIStore {
  private state: UIState = {
    messages: [],
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
    this.listeners.forEach(l => l());
  }

  getState(): UIState {
    return this.state;
  }

  // --- Toast ---
  showToast(content: string, type: MessageType = "info", duration = 3000) {
    const message =
      typeof content === "string" ? content : formatErrorMessage(content);
    this.state = {
      ...this.state,
      messages: [
        ...this.state.messages,
        {
          id: crypto.randomUUID(),
          content: message,
          type,
          duration,
        },
      ],
    };
    this.emit();
  }

  closeToast(id: string) {
    this.state = {
      ...this.state,
      messages: this.state.messages.filter(m => m.id !== id),
    };
    this.emit();
  }

  // --- Modal Stack ---
  showDialog(options: DialogOptions) {
    this.state = {
      ...this.state,
      modals: [
        ...this.state.modals,
        {
          id: crypto.randomUUID(),
          type: "confirm",
          options,
        },
      ],
    };
    this.emit();
  }

  confirm(options: Omit<DialogOptions, "onConfirm" | "onCancel">): Promise<boolean> {
    return new Promise((resolve) => {
      this.showDialog({
        ...options,
        onConfirm: () => resolve(true),
        onCancel: () => resolve(false),
      });
    });
  }

  /**
   * Tri-state confirm with three actions: confirm / discard / cancel.
   * Use for destructive flows where the user can either commit, abandon, or back out.
   */
  confirm3(
    options: Omit<DialogOptions, "onConfirm" | "onCancel" | "onDiscard"> & { discardText: string }
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
    this.state = {
      ...this.state,
      modals: [
        ...this.state.modals,
        {
          id: crypto.randomUUID(),
          type: "import",
          options,
        },
      ],
    };
    this.emit();
  }

  showSqliteTableSelectDialog(options: SqliteTableSelectDialogOptions) {
    this.state = {
      ...this.state,
      modals: [
        ...this.state.modals,
        {
          id: crypto.randomUUID(),
          type: "sqliteTableSelect",
          options,
        },
      ],
    };
    this.emit();
  }

  showExcelSheetSelectDialog(options: ExcelSheetSelectDialogOptions) {
    this.state = {
      ...this.state,
      modals: [
        ...this.state.modals,
        {
          id: crypto.randomUUID(),
          type: "excelSheetSelect",
          options,
        },
      ],
    };
    this.emit();
  }

  showSqlConnectionDialog(options: SqlConnectionDialogOptions) {
    this.state = {
      ...this.state,
      modals: [
        ...this.state.modals,
        {
          id: crypto.randomUUID(),
          type: "sqlConnection",
          options,
        },
      ],
    };
    this.emit();
  }

  showSqlRemoteTableSelectDialog(options: SqlRemoteTableSelectDialogOptions) {
    this.state = {
      ...this.state,
      modals: [
        ...this.state.modals,
        {
          id: crypto.randomUUID(),
          type: "sqlRemoteTableSelect",
          options,
        },
      ],
    };
    this.emit();
  }

  closeModal(id?: string) {
    if (!this.state.modals.length) return;

    let newModals: UIModal[];
    if (!id) {
      newModals = this.state.modals.slice(0, -1);
    } else {
      newModals = this.state.modals.filter(m => m.id !== id);
    }

    this.state = {
      ...this.state,
      modals: newModals,
    };
    this.emit();
  }

  // --- Progress Overlay ---
  /** 启动全局进度蒙层；同一时刻只有一个进度任务。 */
  startProgress(
    progress: ProgressState,
    options?: { onCancel?: () => void },
  ) {
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
