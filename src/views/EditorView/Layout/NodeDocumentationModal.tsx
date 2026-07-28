import { useTranslation } from 'react-i18next';
import { VscClose } from 'react-icons/vsc';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { NODE_CATALOG_UNAVAILABLE_MESSAGE } from '@/features/application/editor/editorMutationAvailability';

interface NodeDocumentationModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function NodeDocumentationModal({ open, onOpenChange }: NodeDocumentationModalProps) {
  const { t } = useTranslation();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-w-lg flex-col gap-0 p-0">
        <DialogHeader className="border-b border-border px-5 py-4">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0 space-y-1">
              <DialogTitle className="normal-case tracking-normal">
                {t('nodeDocumentationModal.title')}
              </DialogTitle>
              <DialogDescription>{NODE_CATALOG_UNAVAILABLE_MESSAGE}</DialogDescription>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onClick={() => onOpenChange(false)}
              aria-label={t('nodeDocumentationModal.close')}
            >
              <VscClose size={18} />
            </Button>
          </div>
        </DialogHeader>
      </DialogContent>
    </Dialog>
  );
}
