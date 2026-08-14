import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import type { ResolvedPinSpec } from '../resolveNodePinSpecs';
import { NodePinSpecRow } from './NodePinSpecRow';
import { detailEmptyHintClass } from '../shared/detailStyles';
import { DetailCollapsibleSection } from '../shared/DetailCollapsibleSection';


interface NodePinInterfacePanelProps {
  graphPath: string;
  inputs: ResolvedPinSpec[];
  outputs: ResolvedPinSpec[];
}

type PinTab = 'inputs' | 'outputs';

function PinList({
  graphPath,
  emptyLabel,
  pins,
}: {
  graphPath: string;
  emptyLabel: string;
  pins: ResolvedPinSpec[];
}) {
  return (
    <div className="space-y-1">
      {pins.length > 0 ? (
        pins.map((pin) => (
          <NodePinSpecRow
            key={pin.id}
            graphPath={graphPath}
            pin={pin}
          />
        ))
      ) : (
        <div className={detailEmptyHintClass}>{emptyLabel}</div>
      )}
    </div>
  );
}

export function NodePinInterfacePanel({
  graphPath,
  inputs,
  outputs,
}: NodePinInterfacePanelProps) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<PinTab>(() =>
    inputs.length === 0 && outputs.length > 0 ? 'outputs' : 'inputs',
  );
  const totalPins = inputs.length + outputs.length;

  useEffect(() => {
    if (activeTab === 'inputs' && inputs.length === 0 && outputs.length > 0) {
      setActiveTab('outputs');
      return;
    }
    if (activeTab === 'outputs' && outputs.length === 0 && inputs.length > 0) {
      setActiveTab('inputs');
    }
  }, [activeTab, inputs.length, outputs.length]);

  return (
    <DetailCollapsibleSection title={t('detail.nodeDoc.pinInterface')}>
      {totalPins > 0 ? (
        <Tabs value={activeTab} onValueChange={(value) => setActiveTab(value as PinTab)}>
          <TabsList className="grid w-full grid-cols-2">
            <TabsTrigger value="inputs" className="gap-1">
              {t('detail.nodeDoc.inputs')}
              <span className="text-[10px] text-muted-foreground">{inputs.length}</span>
            </TabsTrigger>
            <TabsTrigger value="outputs" className="gap-1">
              {t('detail.nodeDoc.outputs')}
              <span className="text-[10px] text-muted-foreground">{outputs.length}</span>
            </TabsTrigger>
          </TabsList>
          <TabsContent value="inputs" className="mt-2">
            <PinList
              graphPath={graphPath}
              emptyLabel={t('detail.nodeDoc.noInputs')}
              pins={inputs}
            />
          </TabsContent>
          <TabsContent value="outputs" className="mt-2">
            <PinList
              graphPath={graphPath}
              emptyLabel={t('detail.nodeDoc.noOutputs')}
              pins={outputs}
            />
          </TabsContent>
        </Tabs>
      ) : (
        <div className={detailEmptyHintClass}>
          {t('detail.nodeDoc.noInputs')} / {t('detail.nodeDoc.noOutputs')}
        </div>
      )}
    </DetailCollapsibleSection>
  );
}
