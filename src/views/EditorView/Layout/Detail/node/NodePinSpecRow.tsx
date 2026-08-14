import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

import type { ResolvedPinSpec } from '../resolveNodePinSpecs';
import { detailPinRowClass } from '../shared/detailStyles';
import { DetailBadge, DetailText } from '../shared/DetailText';

import {
  buildPinViewParams,
  evaluatePinViewState,
  pinViewDisabledTitle,
} from '@/features/core/execution';
import { PinHistoryMenu } from '@/features/application/execution/PinHistoryMenu';
import { useGraphDataStore } from '@/features/core/dataStore';
import { useShallow } from 'zustand/react/shallow';

interface NodePinSpecRowProps {
  graphPath: string;
  pin: ResolvedPinSpec;

}

export function NodePinSpecRow({
  graphPath,
  pin,
}: NodePinSpecRowProps) {
  const { t } = useTranslation();
  const connections = useGraphDataStore(useShallow((state) =>
    pin.connectionIds.flatMap((connectionId) => {
      const connection = state.getGraphConnection(graphPath, connectionId);
      return connection?.output && connection.input
        ? [{
            connectionId: connection.id,
            output: connection.output,
            input: connection.input,
            order: connection.order ?? null,
          }]
        : [];
    }),
  ));

  const viewParams = useMemo(
    () =>
      buildPinViewParams({
        graphPath,
        address: pin.address,
        direction: pin.direction,
        isExec: pin.kind === 'Exec',
        connections,
      }),
    [graphPath, pin.address, pin.direction, pin.kind, connections],
  );

  const viewState = useMemo(
    () => evaluatePinViewState(viewParams),
    [viewParams],
  );

  const showView = viewState.showMenu;
  const viewEnabled = viewState.enabled;
  const viewDisabledLabel = pinViewDisabledTitle(viewState.disabledReason, t);

  const slotNoteText =
    pin.slotNote?.kind === 'repeatableRange'
      ? t('detail.nodeDoc.repeatableRange', {
          min: pin.slotNote.min,
          max: pin.slotNote.max ?? '∞',
        })
      : pin.slotNote?.kind === 'derivedFromInput'
        ? t('detail.nodeDoc.derivedFromInput')
        : undefined;
  const typeLabel = pin.typeLabel;
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
    viewEnabled ? (
      <PinHistoryMenu
        graphPath={graphPath}
        outputs={viewState.refs.flatMap((ref) => ref.kind === 'outputPin' ? [ref.output] : [])}
        label={t('detail.nodeDoc.view')}
      />
    ) : (
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="text-[10px] text-muted-foreground">{t('detail.nodeDoc.view')}</span>
        </TooltipTrigger>
        {viewDisabledLabel ? <TooltipContent side="top">{viewDisabledLabel}</TooltipContent> : null}
      </Tooltip>
    )
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
