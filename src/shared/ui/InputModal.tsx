import { type FormEvent, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { InputDialogOptions } from "@/shared/types/ui";

export const InputModal = ({ options, onClose }: { options: InputDialogOptions; onClose: () => void }) => {
  const [value, setValue] = useState(options.defaultValue ?? "");

  const handleCancel = () => {
    options.onCancel?.();
    onClose();
  };

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();
    options.onSubmit(value);
    onClose();
  };

  return (
    <Dialog open onOpenChange={(open) => !open && handleCancel()}>
      <DialogContent className="max-w-[440px]">
      <form
        onSubmit={handleSubmit}
        className="overflow-hidden"
      >
        <DialogHeader className="border-b border-border bg-muted/20">
          <DialogTitle>{options.title}</DialogTitle>
          {options.message && <DialogDescription>{options.message}</DialogDescription>}
        </DialogHeader>
        <div className="space-y-3 px-6 py-6">
          {options.label && <Label>{options.label}</Label>}
          <Input
            autoFocus
            value={value}
            placeholder={options.placeholder}
            onChange={(event) => setValue(event.target.value)}
          />
        </div>
        <DialogFooter>
          <Button type="button" onClick={handleCancel} variant="ghost" size="lg">
            {options.cancelText || "取消"}
          </Button>
          <Button type="submit" size="lg">
            {options.confirmText || "确定"}
          </Button>
        </DialogFooter>
      </form>
      </DialogContent>
    </Dialog>
  );
};
