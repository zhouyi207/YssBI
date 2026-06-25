import { useTranslation } from 'react-i18next';
import type { ResolvedPinSpec } from '../resolveNodePinSpecs';
import { NodePinSpecRow } from './NodePinSpecRow';

interface NodePinInterfacePanelProps {
  inputs: ResolvedPinSpec[];
  outputs: ResolvedPinSpec[];
}

function PinSection({
  title,
  emptyLabel,
  pins,
}: {
  title: string;
  emptyLabel: string;
  pins: ResolvedPinSpec[];
}) {
  return (
    <div className="px-2 pt-3">
      <div className="mb-1 text-[10px] font-black uppercase tracking-wider text-gray-400">{title}</div>
      <div className="space-y-1">
        {pins.length > 0 ? (
          pins.map((pin) => <NodePinSpecRow key={pin.id} pin={pin} />)
        ) : (
          <div className="py-1 text-center text-[9px] italic text-gray-500">{emptyLabel}</div>
        )}
      </div>
    </div>
  );
}

export function NodePinInterfacePanel({ inputs, outputs }: NodePinInterfacePanelProps) {
  const { t } = useTranslation();

  return (
    <div className="border-t border-white/5">
      <div className="px-2 pt-3 text-[10px] font-black uppercase tracking-widest text-gray-500">
        {t('detail.nodeDoc.pinInterface')}
      </div>
      <PinSection
        title={t('detail.nodeDoc.inputs')}
        emptyLabel={t('detail.nodeDoc.noInputs')}
        pins={inputs}
      />
      <PinSection
        title={t('detail.nodeDoc.outputs')}
        emptyLabel={t('detail.nodeDoc.noOutputs')}
        pins={outputs}
      />
    </div>
  );
}
