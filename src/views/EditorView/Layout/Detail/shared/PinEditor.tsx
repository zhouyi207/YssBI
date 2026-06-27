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
import { DetailCommitInput } from './DetailForm';
import { DetailSectionHeader, DetailText } from './DetailText';

interface PinEditorProps {
  title: string;
  emptyMessage: string;
  pins: Array<{ id: string; name: string; type: string; containerType?: string }>;
  onChange: (pins: PinEditorProps['pins']) => void;
}

export function PinEditor({ title, emptyMessage, pins, onChange }: PinEditorProps) {
  const { t } = useTranslation();

  const containerLabel = (containerType?: string) => {
    if (!containerType) return t('detail.pinEditor.containerNone');
    return containerType;
  };

  return (
    <Card className="rounded-lg bg-card/80 py-0 shadow-xs">
      <CardHeader className="px-3 py-2">
        <div className="flex items-center justify-between">
          <DetailSectionHeader level="subsection">{title}</DetailSectionHeader>
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={() => {
              onChange([
                ...pins,
                { id: `pin-${crypto.randomUUID()}`, name: t('detail.pinEditor.newPin'), type: 'int' },
              ]);
            }}
            className="text-muted-foreground hover:text-[var(--accent-color)]"
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
              <path d="M12 5v14M5 12h14" />
            </svg>
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-1 px-3 pb-3 pt-0">
        <div className="space-y-1">
          {pins.map((pin, idx) => (
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
                value={pin.type}
                options={['exec', 'int', 'float', 'bool', 'string', 'object']}
                onChange={(val) => {
                  const newPins = [...pins];
                  newPins[idx] = { ...newPins[idx], type: val };
                  onChange(newPins);
                }}
              />
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-xs"
                    onClick={() => {
                      const newPins = [...pins];
                      const current = newPins[idx].containerType;
                      const next = current === 'array' ? 'dataseries' : current === 'dataseries' ? undefined : 'array';
                      newPins[idx] = { ...newPins[idx], containerType: next };
                      onChange(newPins);
                    }}
                    className={pin.containerType ? 'bg-blue-500/10 text-blue-400' : 'text-muted-foreground'}
                  >
                    <DetailText tone="smallMuted" className="font-black">
                      {pin.containerType === 'dataseries' ? '◇' : pin.containerType === 'array' ? '[]' : '·'}
                    </DetailText>
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="top">
                  {t('detail.pinEditor.containerTooltip', {
                    container: containerLabel(pin.containerType),
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
                className="opacity-0 transition-opacity group-hover:opacity-100 hover:text-red-500"
              >
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                  <path d="M18 6L6 18M6 6l12 12" />
                </svg>
              </Button>
            </div>
          ))}
          {pins.length === 0 && <div className={detailEmptyHintClass}>{emptyMessage}</div>}
        </div>
      </CardContent>
    </Card>
  );
}
