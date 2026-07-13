import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import type { SidebarInputDialogState } from '../sidebarContextMenu/sidebarContextMenuTypes';

export function SidebarRenameDialog({
  dialog,
  cancelLabel,
  onCancel,
  onSubmit,
  onValueChange,
}: {
  dialog: SidebarInputDialogState | null;
  cancelLabel: string;
  onCancel: () => void;
  onSubmit: () => void;
  onValueChange: (value: string) => void;
}) {
  const { t } = useTranslation();

  return (
    <Dialog open={!!dialog} onOpenChange={(open) => !open && onCancel()}>
      {dialog && (
        <DialogContent className="max-w-[320px]">
          <DialogHeader className="border-b border-border bg-muted/20">
            <DialogTitle>{dialog.title}</DialogTitle>
          </DialogHeader>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void onSubmit();
            }}
          >
            <div className="px-5 py-4">
              <Input
                autoFocus
                value={dialog.value}
                onChange={(e) => onValueChange(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Escape') onCancel();
                }}
                className="h-8 text-xs"
                aria-invalid={!!dialog.error}
              />
              {dialog.error ? (
                <p className="mt-2 text-xs text-destructive" role="alert">
                  {dialog.error}
                </p>
              ) : null}
            </div>
            <DialogFooter>
              <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
                {cancelLabel}
              </Button>
              <Button type="submit" size="sm">
                {dialog.submitLabel ?? t('common.ok')}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      )}
    </Dialog>
  );
}
