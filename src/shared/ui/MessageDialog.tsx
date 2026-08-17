import { VscError, VscInfo, VscWarning } from 'react-icons/vsc';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import type { MessageDialogOptions } from '@/shared/types/ui';

export function MessageDialog({
  options,
  onClose,
}: {
  options: MessageDialogOptions;
  onClose: () => void;
}) {
  const variant = options.type === 'error'
    ? 'destructive'
    : options.type === 'warning' ? 'warning' : 'info';
  const Icon = options.type === 'error'
    ? VscError
    : options.type === 'warning' ? VscWarning : VscInfo;

  return (
    <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="max-w-md border-border bg-card text-card-foreground ring-border sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{options.title}</DialogTitle>
          <DialogDescription className="sr-only">{options.message}</DialogDescription>
        </DialogHeader>
        <div className="px-6 pb-5">
          <Alert variant={variant}>
            <Icon aria-hidden="true" />
            <AlertDescription className="whitespace-pre-wrap text-foreground">
              {options.message}
              {options.incidentId ? (
                <span className="mt-2 block font-mono text-xs text-muted-foreground">
                  {options.incidentLabel}: {options.incidentId}
                </span>
              ) : null}
            </AlertDescription>
          </Alert>
        </div>
        <DialogFooter>
          <Button type="button" onClick={onClose}>{options.closeText}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
