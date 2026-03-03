// ui/UIHost.tsx
import { useUIStore } from "@/features/core/ui/useUIStore";
import { uiStore } from "@/features/core/ui/UIStore";
import { Toast } from "./Toast";
import { Modal } from "./Modal";
import { ImportModal } from "./ImportModal";
import { SqliteTableSelectModal } from "./SqliteTableSelectModal";
import { ExcelSheetSelectModal } from "./ExcelSheetSelectModal";
import { SqlConnectionModal } from "./SqlConnectionModal";
import { SqlRemoteTableSelectModal } from "./SqlRemoteTableSelectModal";

export const UIHost = () => {
  const messages = useUIStore((s) => s.messages);
  const modals = useUIStore((s) => s.modals);
  const top = modals[modals.length - 1];

  return (
    <>
      {/* Toast */}
      <div className="fixed bottom-6 right-6 z-[600] flex flex-col gap-3">
        {messages.map(m => (
          <Toast
            key={m.id}
            message={m}
            onClose={() => uiStore.closeToast(m.id)}
          />
        ))}
      </div>

      {/* Modal Stack */}
      {top?.type === "confirm" && (
        <Modal
          options={top.options}
          onClose={() => uiStore.closeModal(top.id)}
        />
      )}

      {top?.type === "import" && (
        <ImportModal
          options={top.options}
          onClose={() => uiStore.closeModal(top.id)}
        />
      )}

      {top?.type === "sqliteTableSelect" && (
        <SqliteTableSelectModal
          options={top.options}
          onClose={() => uiStore.closeModal(top.id)}
        />
      )}

      {top?.type === "excelSheetSelect" && (
        <ExcelSheetSelectModal
          options={top.options}
          onClose={() => uiStore.closeModal(top.id)}
        />
      )}

      {top?.type === "sqlConnection" && (
        <SqlConnectionModal
          options={top.options}
          onClose={() => uiStore.closeModal(top.id)}
        />
      )}

      {top?.type === "sqlRemoteTableSelect" && (
        <SqlRemoteTableSelectModal
          options={top.options}
          onClose={() => uiStore.closeModal(top.id)}
        />
      )}
    </>
  );
};
