import { useTranslation } from 'react-i18next';
import { Select } from '@/shared/ui';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import {
  detailEmptyHintClass,
  detailInlineInputSmallClass,
  detailPinRowClass,
} from './detailStyles';
import type { FunctionSignaturePin } from '@/shared/types/domain/graph';
import {
  SIGNATURE_EDITOR_TYPE_OPTIONS,
  applySignatureEditorType,
  createDefaultDataSignaturePin,
  cycleSignatureContainer,
  signatureContainerOverlay,
  signatureEditorTypeOption,
} from '@/shared/types/domain/functionSignaturePin';
import { DetailCommitInput } from './DetailForm';
import { DetailSectionHeader, DetailText } from './DetailText';

interface PinEditorProps {
  title: string;
  emptyMessage: string;
  pins: FunctionSignaturePin[];
  onChange: (pins: FunctionSignaturePin[]) => void;
}

export function PinEditor({ title, emptyMessage, pins, onChange }: PinEditorProps) {
  const { t } = useTranslation();

  const containerLabel = (overlay?: 'array' | 'dataseries') => {
    if (!overlay) return t('detail.pinEditor.containerNone');
    return overlay;
  };

  return (
    <Card className="rounded-none border-0 bg-transparent py-0 shadow-none">
      <CardHeader className="h-7 border-0 px-3 py-0">
        <div className="flex w-full items-center justify-between">
          <DetailSectionHeader level="subsection">{title}</DetailSectionHeader>
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={() => {
              onChange([
                ...pins,
                createDefaultDataSignaturePin(
                  `pin-${crypto.randomUUID()}`,
                  t('detail.pinEditor.newPin'),
                ),
              ]);
            }}
            className="text-muted-foreground hover:text-primary"
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
              <path d="M12 5v14M5 12h14" />
            </svg>
          </Button>
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-1 px-3 pb-2 pt-1">
        <div className="flex flex-col gap-1">
          {pins.map((pin, idx) => {
            const editorType = signatureEditorTypeOption(pin);
            const container = signatureContainerOverlay(pin.dataType);
            const isExec = editorType === 'exec';

            return (
              <div key={pin.id} className={detailPinRowClass}>
                <DetailCommitInput
                  className={detailInlineInputSmallClass}
                  value={pin.name}
                  onCommit={(name) => {
                    const newPins = [...pins];
                    newPins[idx] = { ...newPins[idx], name };
                    onChange(newPins);
                  }}
                />
                <Select
                  className="w-24"
                  value={editorType}
                  options={[...SIGNATURE_EDITOR_TYPE_OPTIONS]}
                  onChange={(val) => {
                    const newPins = [...pins];
                    newPins[idx] = applySignatureEditorType(pin, val as typeof editorType);
                    onChange(newPins);
                  }}
                />
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-xs"
                      disabled={isExec}
                      onClick={() => {
                        const newPins = [...pins];
                        newPins[idx] = cycleSignatureContainer(pin);
                        onChange(newPins);
                      }}
                      className={container ? 'bg-primary/10 text-primary' : 'text-muted-foreground'}
                    >
                      <DetailText tone="smallMuted" className="font-black">
                        {container === 'dataseries' ? '◇' : container === 'array' ? '[]' : '·'}
                      </DetailText>
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent side="top">
                    {t('detail.pinEditor.containerTooltip', {
                      container: containerLabel(container),
                    })}
                  </TooltipContent>
                </Tooltip>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => {
                    onChange(pins.filter((_, i) => i !== idx));
                  }}
                  className="opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100 focus-visible:opacity-100 hover:text-destructive"
                >
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                    <path d="M18 6L6 18M6 6l12 12" />
                  </svg>
                </Button>
              </div>
            );
          })}
          {pins.length === 0 && <div className={detailEmptyHintClass}>{emptyMessage}</div>}
        </div>
      </CardContent>
    </Card>
  );
}
