import { applicationUi, useApplicationUiRead } from "@/features/application/ui/applicationUi";

import {
  ExcelSheetSelectModal,
  ImportModal,
  InputModal,
  MessageDialog,
  Modal,
  ProgressOverlay,
  SqlConnectionModal,
  SqliteTableSelectModal,
  SqlRemoteTableSelectModal,

} from "@/shared/ui";

export const UIHost = () => {
  const { modals, progress } = useApplicationUiRead();
  const top = modals[modals.length - 1];

  return (
    <>

      {progress && <ProgressOverlay progress={progress} onCancel={applicationUi.cancelProgress} />}

      {top?.type === "message" && (
        <MessageDialog key={top.id} options={top.options} onClose={() => applicationUi.closeModal(top.id)} />
      )}

      {top?.type === "confirm" && (
        <Modal key={top.id} options={top.options} onClose={() => applicationUi.closeModal(top.id)} />
      )}

      {top?.type === "input" && (
        <InputModal key={top.id} options={top.options} onClose={() => applicationUi.closeModal(top.id)} />
      )}

      {top?.type === "import" && (
        <ImportModal key={top.id} options={top.options} onClose={() => applicationUi.closeModal(top.id)} />
      )}

      {top?.type === "sqliteTableSelect" && (
        <SqliteTableSelectModal key={top.id} options={top.options} onClose={() => applicationUi.closeModal(top.id)} />
      )}

      {top?.type === "excelSheetSelect" && (
        <ExcelSheetSelectModal key={top.id} options={top.options} onClose={() => applicationUi.closeModal(top.id)} />
      )}

      {top?.type === "sqlConnection" && (
        <SqlConnectionModal key={top.id} options={top.options} onClose={() => applicationUi.closeModal(top.id)} />
      )}

      {top?.type === "sqlRemoteTableSelect" && (
        <SqlRemoteTableSelectModal key={top.id} options={top.options} onClose={() => applicationUi.closeModal(top.id)} />
      )}
    </>
  );
};
