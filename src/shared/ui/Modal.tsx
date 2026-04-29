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

  const handleConfirm = () => {
    options.onConfirm();
    onClose();
  };

  return (
    <Dialog open onOpenChange={(open) => !open && handleCancel()}>
      <DialogContent className="max-w-[420px]">
        <DialogHeader className="border-b border-border bg-muted/20">
          <DialogTitle>{options.title}</DialogTitle>
        </DialogHeader>
        <div className="px-6 py-5">
          <DialogDescription className="whitespace-pre-line">{options.message}</DialogDescription>
        </div>
        <DialogFooter>
          <Button onClick={handleCancel} variant="ghost" size="lg">
            {options.cancelText || t("common.cancel")}
          </Button>
          <Button onClick={handleConfirm} variant={options.type === "danger" ? "destructive" : "default"} size="lg">
            {options.confirmText || t("common.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};
