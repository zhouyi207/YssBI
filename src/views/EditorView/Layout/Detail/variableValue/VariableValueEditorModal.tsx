import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Label } from '@/components/ui/label';
import { uiStore } from '@/features/core/ui/UIStore';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import type { DataType } from '@/shared/types/domain/dataType';
import type { DataValue } from '@/shared/types/domain/dataValue';
import {
  dataValueToEditableJson,
  isJsonEditableVariableType,
  parseArrayValueFromJson,
  parseDataFrameValueFromJson,
  parseDataSeriesValueFromJson,
  parseObjectValueFromJson,
} from './variableValueUtils';

interface VariableValueEditorModalProps {
  open: boolean;
  onClose: () => void;
  dataType: DataType;
  dataValue: DataValue;
  onSave: (value: DataValue) => void;
}

const jsonTextareaClass =
  'min-h-[220px] w-full resize-y rounded-md border border-input bg-background px-3 py-2 font-mono text-sm outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30';

export function VariableValueEditorModal({
  open,
  onClose,
  dataType,
  dataValue,
  onSave,
}: VariableValueEditorModalProps) {
  const { t } = useTranslation();
  const [jsonDraft, setJsonDraft] = useState('');

  useEffect(() => {
    if (!open || !isJsonEditableVariableType(dataType)) return;
    setJsonDraft(dataValueToEditableJson(dataType, dataValue));
  }, [open, dataType, dataValue]);

  const handleClear = () => {
    onSave({ kind: 'Null' });
    onClose();
  };

  const handleSave = () => {
    let result:
      | { ok: true; value: DataValue }
      | { ok: false; error: string };

    switch (dataType.kind) {
      case 'Array':
        result = parseArrayValueFromJson(jsonDraft, dataType.inner ?? { kind: 'Any' });
        break;
      case 'Object':
        result = parseObjectValueFromJson(jsonDraft);
        break;
      case 'DataFrame':
        result = parseDataFrameValueFromJson(jsonDraft);
        break;
      case 'DataSeries':
        result = parseDataSeriesValueFromJson(jsonDraft);
        break;
      default:
        return;
    }

    if (!result.ok) {
      uiStore.showToast(t(`detail.variableValue.errors.${result.error}`), 'error');
      return;
    }
    onSave(result.value);
    onClose();
  };

  if (!isJsonEditableVariableType(dataType)) {
    return null;
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent className="flex max-h-[85vh] max-w-[560px] flex-col overflow-hidden p-0">
        <DialogHeader className="border-b border-border bg-muted/20 px-6 py-4">
          <DialogTitle>{t(`detail.variableValue.title.${dataType.kind}`)}</DialogTitle>
          <DialogDescription>
            {t(`detail.variableValue.description.${dataType.kind}`)}
          </DialogDescription>
        </DialogHeader>

        <div className="min-h-0 flex-1 px-6 py-5">
          <div className="flex min-h-0 flex-1 flex-col gap-2">
            <Label>{t('detail.variableValue.jsonLabel')}</Label>
            <OverlayScrollbar className="min-h-0 flex-1">
              <textarea
                className={jsonTextareaClass}
                value={jsonDraft}
                spellCheck={false}
                onChange={(event) => setJsonDraft(event.target.value)}
              />
            </OverlayScrollbar>
          </div>
        </div>

        <DialogFooter className="border-t border-border px-6 py-4">
          <Button type="button" variant="ghost" size="lg" onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button type="button" variant="outline" size="lg" onClick={handleClear}>
            {t('detail.variableValue.clear')}
          </Button>
          <Button type="button" size="lg" onClick={handleSave}>
            {t('common.confirm')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
