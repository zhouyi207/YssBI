import { useUIStore } from "@/features/core/ui/useUIStore";
import { uiStore } from "@/features/core/ui/UIStore";

import {
  ExcelSheetSelectModal,
  ImportModal,
  InputModal,
  Modal,
  ProgressOverlay,
  SqlConnectionModal,
  SqliteTableSelectModal,
  SqlRemoteTableSelectModal,

} from "@/shared/ui";

export const UIHost = () => {

  const modals = useUIStore((s) => s.modals);
  const progress = useUIStore((s) => s.progress);
  const top = modals[modals.length - 1];

  return (
    <>

      {progress && <ProgressOverlay progress={progress} />}

      {top?.type === "confirm" && (
        <Modal key={top.id} options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}

      {top?.type === "input" && (
        <InputModal key={top.id} options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}

      {top?.type === "import" && (
        <ImportModal key={top.id} options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}

      {top?.type === "sqliteTableSelect" && (
        <SqliteTableSelectModal key={top.id} options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}

      {top?.type === "excelSheetSelect" && (
        <ExcelSheetSelectModal key={top.id} options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}

      {top?.type === "sqlConnection" && (
        <SqlConnectionModal key={top.id} options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}

      {top?.type === "sqlRemoteTableSelect" && (
        <SqlRemoteTableSelectModal key={top.id} options={top.options} onClose={() => uiStore.closeModal(top.id)} />
      )}
    </>
  );
};
