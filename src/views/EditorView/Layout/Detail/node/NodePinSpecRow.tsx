import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { Button } from '@/components/ui/button';
import type { ResolvedPinSpec } from '../resolveNodePinSpecs';
import { detailPinRowClass } from '../shared/detailStyles';
import { DetailBadge, DetailText } from '../shared/DetailText';
import type { PinResultState } from '@/shared/types/ui';
import {
  buildPinViewParams,
  openPinView,
  pinViewDisabledTitle,
  resolvePinViewDisabledReason,
  resolvePinViewTargetFromCache,
  shouldShowPinViewMenuItem,
} from '@/features/core/execution';
import type { ExecutionStatus } from '@/shared/types/ui';

interface NodePinSpecRowProps {
  graphId: string;
  pin: ResolvedPinSpec;
  pinResults?: Map<string, PinResultState>;
  executionStatus?: ExecutionStatus;
}

export function NodePinSpecRow({
  graphId,
  pin,
  pinResults,
  executionStatus,
}: NodePinSpecRowProps) {
  const { t } = useTranslation();

  const viewParams = useMemo(
    () =>
      buildPinViewParams({
        graphId,
        pinId: pin.id,
        direction: pin.direction,
        pinType: pin.kind === 'Exec' ? 'exec' : pin.type,
        connectionIds: pin.connectionIds,
        pinResults,
        executionStatus,
      }),
    [graphId, pin, pinResults, executionStatus],
  );

  const showView = shouldShowPinViewMenuItem(viewParams);
  const viewTarget = resolvePinViewTargetFromCache(viewParams);
  const viewDisabledReason = resolvePinViewDisabledReason(viewParams);
  const viewEnabled =
    showView &&
    (Boolean(viewTarget) ||
      (executionStatus === 'completed' &&
        (pin.direction === 'output' || (pin.direction === 'input' && pin.connectionIds.length > 0))));
  const viewDisabledLabel = pinViewDisabledTitle(viewDisabledReason, t);

  const slotNoteText =
    pin.slotNote?.kind === 'repeatableRange'
      ? t('detail.nodeDoc.repeatableRange', {
          min: pin.slotNote.min,
          max: pin.slotNote.max ?? '∞',
        })
      : pin.slotNote?.kind === 'derivedFromInput'
        ? t('detail.nodeDoc.derivedFromInput')
        : undefined;
  const typeLabel = pin.typeDisplay ?? pin.type;
  const badges: Array<{ label: string; tooltip?: string }> = [];
  if (pin.optional) badges.push({ label: t('detail.nodeDoc.optional') });
  if (pin.slotKind === 'repeatable') {
    badges.push({ label: t('detail.nodeDoc.repeatable'), tooltip: slotNoteText });
  }
  if (pin.slotKind === 'derivedFromInput') {
    badges.push({ label: t('detail.nodeDoc.derived'), tooltip: slotNoteText });
  }

  const renderBadge = (badge: { label: string; tooltip?: string }) => {
    if (!badge.tooltip) {
      return <DetailBadge key={badge.label}>{badge.label}</DetailBadge>;
    }

    return (
      <Tooltip key={badge.label}>
        <TooltipTrigger asChild>
          <span>
            <DetailBadge>{badge.label}</DetailBadge>
          </span>
        </TooltipTrigger>
        <TooltipContent side="top">
          {badge.tooltip}
        </TooltipContent>
      </Tooltip>
    );
  };

  const viewButton = showView ? (
    <Tooltip>
      <TooltipTrigger asChild>
        <span>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            className="h-5 px-1.5 text-[10px]"
            disabled={!viewEnabled}
            onClick={() => void openPinView(viewParams, t)}
          >
            {t('detail.nodeDoc.view')}
          </Button>
        </span>
      </TooltipTrigger>
      {!viewEnabled && viewDisabledLabel ? (
        <TooltipContent side="top">{viewDisabledLabel}</TooltipContent>
      ) : null}
    </Tooltip>
  ) : null;

  return (
    <div
      className={`border-l-2 border-blue-400/70 px-2 py-1.5 ${detailPinRowClass}`}
    >
      <div className="flex min-w-0 flex-1 items-center justify-between gap-2">
        <Tooltip>
          <TooltipTrigger asChild>
            <div className="flex min-w-0 flex-1 items-center">
              <DetailText className="min-w-0 truncate font-semibold">
                {pin.name || t('detail.nodeDoc.unnamed')}
              </DetailText>
            </div>
          </TooltipTrigger>
          <TooltipContent side="left" className="font-mono">
            {typeLabel}
          </TooltipContent>
        </Tooltip>
        {(pin.connected || badges.length > 0 || showView) && (
          <div className="flex min-w-0 shrink-0 items-center justify-end gap-1">
            {pin.connected && (
              <span
                className="h-1.5 w-1.5 rounded-full bg-emerald-400/80"
                title={t('detail.nodeDoc.connected')}
              />
            )}
            {badges.map(renderBadge)}
            {viewButton}
          </div>
        )}
      </div>
    </div>
  );
}
