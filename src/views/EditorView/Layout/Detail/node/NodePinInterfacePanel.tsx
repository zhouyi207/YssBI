import { useTranslation } from 'react-i18next';
import type { ResolvedPinSpec } from '../resolveNodePinSpecs';
import { NodePinSpecRow } from './NodePinSpecRow';
import { detailEmptyHintClass, detailSectionTitleClass, detailSubsectionTitleClass } from '../shared/detailStyles';

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
      <div className={`mb-1 ${detailSubsectionTitleClass}`}>{title}</div>
      <div className="space-y-1">
        {pins.length > 0 ? (
          pins.map((pin) => <NodePinSpecRow key={pin.id} pin={pin} />)
        ) : (
          <div className={detailEmptyHintClass}>{emptyLabel}</div>
        )}
      </div>
    </div>
  );
}

export function NodePinInterfacePanel({ inputs, outputs }: NodePinInterfacePanelProps) {
  const { t } = useTranslation();

  return (
    <div className="border-t border-border">
      <div className={`px-2 pt-3 ${detailSectionTitleClass}`}>{t('detail.nodeDoc.pinInterface')}</div>
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
