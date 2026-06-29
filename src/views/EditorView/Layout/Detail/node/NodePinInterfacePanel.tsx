import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import type { ResolvedPinSpec } from '../resolveNodePinSpecs';
import { NodePinSpecRow } from './NodePinSpecRow';
import { detailEmptyHintClass } from '../shared/detailStyles';
import { DetailCollapsibleSection } from '../shared/DetailCollapsibleSection';
import type { PinResultState } from '@/shared/types/ui';

interface NodePinInterfacePanelProps {
  inputs: ResolvedPinSpec[];
  outputs: ResolvedPinSpec[];
  pinResults?: Map<string, PinResultState>;
  selectedResultPinId?: string | null;
  onInspectResult?: (pinId: string) => void;
}

type PinTab = 'inputs' | 'outputs';

function PinList({
  emptyLabel,
  pins,
  pinResults,
  selectedResultPinId,
  onInspectResult,
}: {
  emptyLabel: string;
  pins: ResolvedPinSpec[];
  pinResults?: Map<string, PinResultState>;
  selectedResultPinId?: string | null;
  onInspectResult?: (pinId: string) => void;
}) {
  return (
    <div className="space-y-1">
      {pins.length > 0 ? (
        pins.map((pin) => (
          <NodePinSpecRow
            key={pin.id}
            pin={pin}
            result={pinResults?.get(pin.id)}
            selected={selectedResultPinId === pin.id}
            onInspect={onInspectResult}
          />
        ))
      ) : (
        <div className={detailEmptyHintClass}>{emptyLabel}</div>
      )}
    </div>
  );
}

export function NodePinInterfacePanel({
  inputs,
  outputs,
  pinResults,
  selectedResultPinId,
  onInspectResult,
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
            <PinList emptyLabel={t('detail.nodeDoc.noInputs')} pins={inputs} />
          </TabsContent>
          <TabsContent value="outputs" className="mt-2">
            <PinList
              emptyLabel={t('detail.nodeDoc.noOutputs')}
              pins={outputs}
              pinResults={pinResults}
              selectedResultPinId={selectedResultPinId}
              onInspectResult={onInspectResult}
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
