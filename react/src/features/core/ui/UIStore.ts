import { createStore } from "zustand/vanilla";
import { shallow } from "zustand/shallow";
import {
  DialogOptions,
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

export interface ProgressHandle {
  update: (patch: Partial<ProgressState>) => void;
  finish: () => void;
}

class UIStore {
  private readonly store = createStore<ApplicationUiState>(() => ({ modals: [], progress: null }));
  private activeProgress: { handle: ProgressHandle; onCancel?: () => void } | null = null;

  readonly getState = this.store.getState;
  readonly getInitialState = this.store.getInitialState;
  readonly subscribe = this.store.subscribe;

  // --- Modal Stack ---
  showSettings() {
    if (this.getState().modals.some((modal) => modal.type === "settings")) return;
    this.store.setState({
      modals: [...this.getState().modals, { id: crypto.randomUUID(), type: "settings" }],
    });
  }

  private showDialog(options: DialogOptions, parentId?: string) {
    if (parentId && !this.getState().modals.some((modal) => modal.id === parentId)) {
      options.onCancel?.();
      return;
    }
    this.store.setState({
      modals: [
        ...this.getState().modals,
        {
          id: crypto.randomUUID(),
          type: "confirm",
          parentId,
          options,
        },
      ],
    });
  }

  alert(options: MessageDialogOptions): Promise<void> {
    return new Promise((resolve) => {
      const id = crypto.randomUUID();
      const unsubscribe = this.subscribe((state) => {
        if (state.modals.some((modal) => modal.id === id)) return;
        unsubscribe();
        resolve();
      });
      this.store.setState({
        modals: [...this.getState().modals, { id, type: "message", options }],
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

  showImportDialog(options: ImportDialogOptions) {
    const existing = this.getState().modals.find((modal) => modal.type === "import");
    if (existing) return existing.id;
    const id = crypto.randomUUID();
    this.store.setState({
      modals: [
        ...this.getState().modals,
        {
          id,
          type: "import",
          options,
        },
      ],
    });
    return id;
  }

  showSqliteTableSelectDialog(options: SqliteTableSelectDialogOptions, parentId: string) {
    if (!this.getState().modals.some((modal) => modal.id === parentId)) return;
    this.store.setState({
      modals: [
        ...this.getState().modals,
        {
          id: crypto.randomUUID(),
          type: "sqliteTableSelect",
          parentId,
          options,
        },
      ],
    });
  }

  showExcelSheetSelectDialog(options: ExcelSheetSelectDialogOptions, parentId: string) {
    if (!this.getState().modals.some((modal) => modal.id === parentId)) return;
    this.store.setState({
      modals: [
        ...this.getState().modals,
        {
          id: crypto.randomUUID(),
          type: "excelSheetSelect",
          parentId,
          options,
        },
      ],
    });
  }

  showSqlConnectionDialog(options: SqlConnectionDialogOptions, parentId: string) {
    if (!this.getState().modals.some((modal) => modal.id === parentId)) return;
    const id = crypto.randomUUID();
    this.store.setState({
      modals: [
        ...this.getState().modals,
        {
          id,
          type: "sqlConnection",
          parentId,
          options,
        },
      ],
    });
    return id;
  }

  showSqlRemoteTableSelectDialog(options: SqlRemoteTableSelectDialogOptions, parentId: string) {
    if (!this.getState().modals.some((modal) => modal.id === parentId)) return;
    this.store.setState({
      modals: [
        ...this.getState().modals,
        {
          id: crypto.randomUUID(),
          type: "sqlRemoteTableSelect",
          parentId,
          options,
        },
      ],
    });
  }

  closeModal(id: string) {
    if (!this.getState().modals.some((modal) => modal.id === id)) return;
    const removedIds = new Set([id]);
    // Children are appended after their parent; closing a workflow removes only its descendants.
    const newModals = this.getState().modals.filter((modal) => {
      if (modal.parentId && removedIds.has(modal.parentId)) removedIds.add(modal.id);
      return !removedIds.has(modal.id);
    });
    const removedModals = this.getState().modals.filter((modal) => removedIds.has(modal.id));
    this.store.setState({
      modals: newModals,
    });
    // Complete programmatically dismissed confirmations too. A user's decision has already settled its promise.
    for (const modal of removedModals) {
      if (modal.type === "confirm") modal.options.onCancel?.();
    }
  }

  // --- Progress Overlay ---
  /** Only the latest task's handle may update or finish the visible progress. */
  startProgress(progress: ProgressState, options?: { onCancel?: () => void }): ProgressHandle {
    const handle: ProgressHandle = {
      update: (patch) => {
        if (this.activeProgress?.handle !== handle) return;
        const current = this.getState().progress;
        if (!current) return;
        const next = { ...current, ...patch };
        if (!shallow(current, next)) this.store.setState({ progress: next });
      },
      finish: () => {
        if (this.activeProgress?.handle !== handle) return;
        this.activeProgress = null;
        this.store.setState({ progress: null });
      },
    };
    this.activeProgress = { handle, onCancel: options?.onCancel };
    this.store.setState({
      progress: {
        ...progress,
        cancelable: progress.cancelable ?? !!options?.onCancel,
      },
    });
    return handle;
  }

  /** 用户点击进度蒙层关闭按钮时调用。 */
  cancelProgress() {
    const current = this.activeProgress;
    if (!current || !this.getState().progress?.cancelable) return;
    try {
      current.onCancel?.();
    } finally {
      current.handle.finish();
    }
  }
}

export const uiStore = new UIStore();
