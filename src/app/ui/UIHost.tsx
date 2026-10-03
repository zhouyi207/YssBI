import { DialogStackLayer } from "@/components/ui/dialog";
import { SettingsDialog } from "@/modules/settings/public";
import { applicationUi, useApplicationUiRead } from "@/features/application/ui/applicationUi";
import {
  ExcelSheetSelectModal,
  ImportModal,
  SqlConnectionModal,
  SqliteTableSelectModal,
  SqlRemoteTableSelectModal,
} from "@/modules/data-explorer/public";

import { MessageDialog, Modal, ProgressOverlay } from "@/shared/ui";

function ProgressHost() {
  const progress = useApplicationUiRead((state) => state.progress);
  return (
    progress && <ProgressOverlay progress={progress} onCancel={applicationUi.cancelProgress} />
  );
}

function ModalHost() {
  const modals = useApplicationUiRead((state) => state.modals);

  return (
    <>
      {modals.map((modal, index) => (
        <DialogStackLayer key={modal.id} index={index}>
          {modal.type === "settings" && (
            <SettingsDialog modalId={modal.id} onClose={() => applicationUi.closeModal(modal.id)} />
          )}
          {modal.type === "message" && (
            <MessageDialog
              options={modal.options}
              onClose={() => applicationUi.closeModal(modal.id)}
            />
          )}

          {modal.type === "confirm" && (
            <Modal options={modal.options} onClose={() => applicationUi.closeModal(modal.id)} />
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
              modalId={modal.id}
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
}

export const UIHost = () => (
  <>
    <ProgressHost />
    <ModalHost />
  </>
);
