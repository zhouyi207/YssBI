// ui/UIHost.tsx
import { useUIStore } from "@/features/core/ui/useUIStore";
import { uiStore } from "@/features/core/ui/UIStore";
import { Toast } from "@/shared/ui/Toast";
import { Modal } from "@/shared/ui/Modal";
import { ImportModal } from "@/shared/ui/ImportModal";

export const UIHost = () => {
  const { messages, modals } = useUIStore();
  const top = modals[modals.length - 1];

  return (
    <>
      {/* Toast */}
      <div className="fixed bottom-6 right-6 z-[2000] flex flex-col gap-3">
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
    </>
  );
};
