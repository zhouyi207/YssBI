import {
    Message,
    MessageType,
    DialogOptions,
    ImportDialogOptions,
    SqliteTableSelectDialogOptions,
    ExcelSheetSelectDialogOptions,
    SqlConnectionDialogOptions,
    SqlRemoteTableSelectDialogOptions,
} from "@/shared/types/ui";

type UIModal =
    | { id: string; type: "confirm"; options: DialogOptions }
    | { id: string; type: "import"; options: ImportDialogOptions }
    | { id: string; type: "sqliteTableSelect"; options: SqliteTableSelectDialogOptions }
    | { id: string; type: "excelSheetSelect"; options: ExcelSheetSelectDialogOptions }
    | { id: string; type: "sqlConnection"; options: SqlConnectionDialogOptions }
    | { id: string; type: "sqlRemoteTableSelect"; options: SqlRemoteTableSelectDialogOptions };

type UIState = {
  messages: Message[];
  modals: UIModal[];
};

type Listener = () => void;

class UIStore {
  private state: UIState = {
    messages: [],
    modals: [],
  };

  private listeners = new Set<Listener>();

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
    this.state = {
      ...this.state,
      messages: [
        ...this.state.messages,
        {
          id: crypto.randomUUID(),
          content,
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
}

export const uiStore = new UIStore();
