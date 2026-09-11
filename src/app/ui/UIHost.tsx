import { DialogStackLayer } from "@/components/ui/dialog";
import { applicationUi, useApplicationUiRead } from "@/features/application/ui/applicationUi";
import {
  ExcelSheetSelectModal,
  ImportModal,
  SqlConnectionModal,
  SqliteTableSelectModal,
  SqlRemoteTableSelectModal,
} from "@/modules/data-explorer/public";

import { InputModal, MessageDialog, Modal, ProgressOverlay } from "@/shared/ui";

export const UIHost = () => {
  const { modals, progress } = useApplicationUiRead();

  return (
    <>
      {progress && <ProgressOverlay progress={progress} onCancel={applicationUi.cancelProgress} />}

      {modals.map((modal, index) => (
        <DialogStackLayer key={modal.id} index={index}>
          {modal.type === "message" && (
            <MessageDialog
              options={modal.options}
              onClose={() => applicationUi.closeModal(modal.id)}
            />
          )}

          {modal.type === "confirm" && (
            <Modal options={modal.options} onClose={() => applicationUi.closeModal(modal.id)} />
          )}

          {modal.type === "input" && (
            <InputModal
              options={modal.options}
              onClose={() => applicationUi.closeModal(modal.id)}
            />
          )}

          {modal.type === "import" && (
            <ImportModal
              options={modal.options}
              onClose={() => applicationUi.closeModal(modal.id)}
            />
          )}

          {modal.type === "sqliteTableSelect" && (
            <SqliteTableSelectModal
              options={modal.options}
              onClose={() => applicationUi.closeModal(modal.id)}
            />
          )}

          {modal.type === "excelSheetSelect" && (
            <ExcelSheetSelectModal
              options={modal.options}
              onClose={() => applicationUi.closeModal(modal.id)}
            />
          )}

          {modal.type === "sqlConnection" && (
            <SqlConnectionModal
              options={modal.options}
              onClose={() => applicationUi.closeModal(modal.id)}
            />
          )}

          {modal.type === "sqlRemoteTableSelect" && (
            <SqlRemoteTableSelectModal
              options={modal.options}
              onClose={() => applicationUi.closeModal(modal.id)}
            />
          )}
        </DialogStackLayer>
      ))}
    </>
  );
};
