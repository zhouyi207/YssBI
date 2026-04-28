import { useUIStore } from "@/features/core/ui/useUIStore";
import { uiStore } from "@/features/core/ui/UIStore";
import { Toaster } from "@/components/ui/sonner";
import {
  ExcelSheetSelectModal,
  ImportModal,
  InputModal,
  Modal,
  SqlConnectionModal,
  SqliteTableSelectModal,
  SqlRemoteTableSelectModal,
  Toast,
} from "@/shared/ui";

export const UIHost = () => {
  const messages = useUIStore((s) => s.messages);
  const modals = useUIStore((s) => s.modals);
  const top = modals[modals.length - 1];

  return (
    <>
      <Toaster />
      {messages.map((message) => (
        <Toast key={message.id} message={message} onClose={() => uiStore.closeToast(message.id)} />
      ))}

      {top?.type === "confirm" && (
        <Modal options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}

      {top?.type === "input" && (
        <InputModal options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}

      {top?.type === "import" && (
        <ImportModal options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}

      {top?.type === "sqliteTableSelect" && (
        <SqliteTableSelectModal options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}

      {top?.type === "excelSheetSelect" && (
        <ExcelSheetSelectModal options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}

      {top?.type === "sqlConnection" && (
        <SqlConnectionModal options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}

      {top?.type === "sqlRemoteTableSelect" && (
        <SqlRemoteTableSelectModal options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}
    </>
  );
};
