import { useCallback, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { VscChevronDown, VscEye } from 'react-icons/vsc';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { pinHistoryCacheKey, useExecutionStore } from '@/features/core/execution';
import { resultRef } from '@/features/application/results';
import { ResultService } from '@/services/result/resultService';
import type { PortAddressDto } from '@/shared/types/domain/editorProjection';
import type { PinHistoryProjection } from '@/shared/types/ui';
import { openInspectableResult } from './openInspectableResult';

interface PinHistoryMenuProps {
  graphPath: string;
  outputs: readonly PortAddressDto[];
  label?: ReactNode;
  className?: string;
}

function formatCreatedAt(createdAtMs: string): string {
  const timestamp = Number(createdAtMs);
  if (!Number.isFinite(timestamp)) return createdAtMs;
  return new Date(timestamp).toLocaleString();
}

export function PinHistoryMenu({
  graphPath,
  outputs,
  label,
  className,
}: PinHistoryMenuProps) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);
  const [projections, setProjections] = useState<PinHistoryProjection[]>([]);

  const loadHistory = useCallback(async () => {
    if (outputs.length === 0) return;
    setLoading(true);
    try {
      const histories = await Promise.all(outputs.map(async (output): Promise<PinHistoryProjection> => {
        const entries = await ResultService.getPinHistory(graphPath, output);
        return {
          graphPath,
          output,
          entries,
          selectedResultId: entries[entries.length - 1]?.resultId ?? null,
        };
      }));
      setProjections(histories);
      const store = useExecutionStore.getState();
      histories.forEach((history) => store.recordPinHistory(history));
    } finally {
      setLoading(false);
    }
  }, [graphPath, outputs]);

  const entries = projections.flatMap((projection) =>
    projection.entries.map((entry, index) => ({
      entry,
      projection,
      latest: index === projection.entries.length - 1,
    })).reverse(),
  );

  const openEntry = useCallback((projection: PinHistoryProjection, resultId: string) => {
    useExecutionStore.getState().recordPinHistory({ ...projection, selectedResultId: resultId });
    void openInspectableResult(resultRef(resultId), t);
  }, [t]);

  return (
    <DropdownMenu onOpenChange={(open) => { if (open) void loadHistory(); }}>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          className={className ?? 'h-5 gap-1 rounded-sm px-1.5 text-[10px]'}
          disabled={outputs.length === 0}
          aria-label={t('contextMenu.pin.history')}
        >
          <VscEye size={12} />
          {label}
          <VscChevronDown size={10} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-56 rounded-sm py-0">
        {loading ? (
          <DropdownMenuItem disabled className="py-1 text-xs">
            {t('contextMenu.pin.historyLoading')}
          </DropdownMenuItem>
        ) : entries.length === 0 ? (
          <DropdownMenuItem disabled className="py-1 text-xs">
            {t('contextMenu.pin.historyEmpty')}
          </DropdownMenuItem>
        ) : entries.map(({ entry, projection, latest }) => (
          <DropdownMenuItem
            key={`${pinHistoryCacheKey(graphPath, projection.output)}:${entry.activationId}`}
            className="items-start gap-2 py-1 text-xs"
            onSelect={() => openEntry(projection, entry.resultId)}
          >
            <span className="min-w-0 flex-1">
              <span className="flex items-center gap-1">
                <span className="font-medium">{entry.resultId}</span>
                <span className="text-muted-foreground">{entry.state.kind}</span>
                {latest ? <span className="text-[10px] text-primary">{t('contextMenu.pin.historyLatest')}</span> : null}
              </span>
              <span className="block truncate text-[10px] text-muted-foreground">
                {formatCreatedAt(entry.createdAtMs)} · {entry.runId}
              </span>
            </span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
