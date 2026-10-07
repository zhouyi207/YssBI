import { useTranslation } from "react-i18next";
import { DialogOptions } from "@/shared/types/ui";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export const Modal = ({ options, onClose }: { options: DialogOptions; onClose: () => void }) => {
  const { t } = useTranslation();
  const handleCancel = () => {
    options.onCancel?.();
    onClose();
  };

  const handleDiscard = () => {
    options.onDiscard?.();
    onClose();
  };

  const handleConfirm = () => {
    options.onConfirm();
    onClose();
  };

  const hasDiscard = !!options.discardText;

  return (
    <Dialog open onOpenChange={(open) => !open && handleCancel()}>
      <DialogContent className="flex max-h-[calc(100dvh-2rem)] w-[calc(100vw-2rem)] max-w-[420px] flex-col">
        <DialogHeader className="min-w-0 shrink-0 border-b border-border bg-muted/20">
          <DialogTitle className="[overflow-wrap:anywhere]">{options.title}</DialogTitle>
        </DialogHeader>
        <div className="min-h-0 min-w-0 flex-1 overflow-y-auto overscroll-contain px-6 py-5">
          <DialogDescription className="whitespace-pre-line [overflow-wrap:anywhere]">
            {options.message}
          </DialogDescription>
        </div>
        <DialogFooter className="shrink-0 flex-wrap">
          <Button onClick={handleCancel} variant="ghost" size="lg">
            {options.cancelText || t("common.cancel")}
          </Button>
          {hasDiscard && (
            <Button onClick={handleDiscard} variant="outline" size="lg">
              {options.discardText}
            </Button>
          )}
          <Button
            onClick={handleConfirm}
            variant={options.type === "danger" ? "destructive" : "default"}
            size="lg"
          >
            {options.confirmText || t("common.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};
