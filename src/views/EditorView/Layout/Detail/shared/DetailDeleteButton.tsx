import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { uiStore } from '@/features/core/ui/UIStore';

interface DetailDeleteButtonProps {
  itemType: 'variable' | 'event' | 'function' | 'data' | 'worksheet';
  itemName: string;
  onDelete: () => Promise<void> | void;
  onDeleted?: () => void;
}

export function DetailDeleteButton({
  itemType,
  itemName,
  onDelete,
  onDeleted,
}: DetailDeleteButtonProps) {
  const { t } = useTranslation();
  const itemTypeLabel = t(`detail.itemTypes.${itemType}`);

  return (
    <div className="p-2">
      <Button
        type="button"
        variant="destructive"
        size="sm"
        onClick={() => {
          uiStore.showDialog({
            title: t('detail.delete.title', { itemType: itemTypeLabel }),
            message: t('detail.delete.message', { itemType: itemTypeLabel, name: itemName }),
            type: 'danger',
            confirmText: t('common.delete'),
            onConfirm: async () => {
              await onDelete();
              onDeleted?.();
            },
          });
        }}
        className="mt-4 w-full uppercase tracking-wider"
      >
        {t('detail.delete.button', { itemType: itemTypeLabel })}
      </Button>
    </div>
  );
}
