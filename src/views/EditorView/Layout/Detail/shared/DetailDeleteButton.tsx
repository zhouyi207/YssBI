import { Button } from '@/components/ui/button';
import { uiStore } from '@/features/core/ui/UIStore';

interface DetailDeleteButtonProps {
  itemType: string;
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
  return (
    <div className="p-2">
      <Button
        type="button"
        variant="destructive"
        size="sm"
        onClick={() => {
          uiStore.showDialog({
            title: `Delete ${itemType}`,
            message: `Are you sure you want to delete ${itemType} '${itemName}'?`,
            type: 'danger',
            confirmText: 'Delete',
            onConfirm: async () => {
              await onDelete();
              onDeleted?.();
            },
          });
        }}
        className="mt-4 w-full uppercase tracking-wider"
      >
        Delete {itemType}
      </Button>
    </div>
  );
}
