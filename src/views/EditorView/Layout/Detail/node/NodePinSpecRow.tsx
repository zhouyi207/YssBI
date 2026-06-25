import { useTranslation } from 'react-i18next';
import type { ResolvedPinSpec } from '../resolveNodePinSpecs';

interface NodePinSpecRowProps {
  pin: ResolvedPinSpec;
}

export function NodePinSpecRow({ pin }: NodePinSpecRowProps) {
  const { t } = useTranslation();

  const badges: string[] = [];
  if (pin.optional) badges.push(t('detail.nodeDoc.optional'));
  else badges.push(t('detail.nodeDoc.required'));
  if (pin.slotKind === 'repeatable') badges.push(t('detail.nodeDoc.repeatable'));
  if (pin.slotKind === 'derivedFromInput') badges.push(t('detail.nodeDoc.derived'));
  if (pin.connected) badges.push(t('detail.nodeDoc.connected'));

  const directionLabel =
    pin.direction === 'input'
      ? t('detail.nodeDoc.directionInput')
      : t('detail.nodeDoc.directionOutput');

  const kindLabel = pin.kind === 'Exec' ? t('detail.nodeDoc.kindExec') : t('detail.nodeDoc.kindData');

  const slotNoteText =
    pin.slotNote?.kind === 'repeatableRange'
      ? t('detail.nodeDoc.repeatableRange', {
          min: pin.slotNote.min,
          max: pin.slotNote.max ?? '∞',
        })
      : pin.slotNote?.kind === 'derivedFromInput'
        ? t('detail.nodeDoc.derivedFromInput')
        : undefined;

  return (
    <div className="flex items-start gap-2 rounded bg-white/5 px-2 py-1.5">
      <span
        className={`mt-0.5 shrink-0 rounded px-1 py-0.5 text-[8px] font-black uppercase ${
          pin.direction === 'input' ? 'bg-blue-500/20 text-blue-300' : 'bg-emerald-500/20 text-emerald-300'
        }`}
      >
        {directionLabel}
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-1">
          <span className="text-[10px] font-semibold text-gray-200">
            {pin.name || t('detail.nodeDoc.unnamed')}
          </span>
          <span className="text-[9px] text-gray-500">{kindLabel}</span>
        </div>
        <div className="mt-0.5 font-mono text-[9px] text-[var(--accent-color)]/80">
          {pin.typeDisplay ?? pin.type}
        </div>
        {(slotNoteText || badges.length > 0) && (
          <div className="mt-1 flex flex-wrap gap-1">
            {badges.map((badge) => (
              <span
                key={badge}
                className="rounded bg-black/30 px-1 py-0.5 text-[8px] uppercase tracking-wide text-gray-400"
              >
                {badge}
              </span>
            ))}
            {slotNoteText && (
              <span className="text-[8px] italic text-gray-500">{slotNoteText}</span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
