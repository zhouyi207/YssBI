import { useTranslation } from 'react-i18next';
import type { BayesModelDraftDTO, InferenceConfigDTO } from '@/shared/types/bayes';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { EditableNumberField, PanelTitle, ReadOnlyField } from './BayesFields';

export function SamplerStep({ draft, onSamplerChange }: { draft: BayesModelDraftDTO; onSamplerChange: (sampler: InferenceConfigDTO) => void }) {
  const { t } = useTranslation();
  const updateNumber = (key: keyof InferenceConfigDTO, value: string) => {
    const numberValue = Number(value);
    if (!Number.isFinite(numberValue)) return;
    onSamplerChange({ ...draft.sampler, [key]: numberValue });
  };

  return (
    <Card>
      <CardHeader><PanelTitle title={t('bayes.sampler.title')} description={t('bayes.sampler.nutsOnlyDescription')} /></CardHeader>
            <CardContent className="grid grid-cols-2 gap-3 lg:grid-cols-3">
              <ReadOnlyField label={t('bayes.sampler.algorithm')} value={draft.sampler.algorithm.toUpperCase()} />
              <EditableNumberField label={t('bayes.sampler.chains')} value={draft.sampler.chains} min={1} onChange={(value) => updateNumber('chains', value)} />
              <EditableNumberField label={t('bayes.sampler.samples')} value={draft.sampler.samples} min={1} onChange={(value) => updateNumber('samples', value)} />
              <EditableNumberField label={t('bayes.sampler.warmup')} value={draft.sampler.warmup} min={0} onChange={(value) => updateNumber('warmup', value)} />
              <EditableNumberField label={t('bayes.sampler.seed')} value={draft.sampler.seed ?? 1234} min={0} onChange={(value) => updateNumber('seed', value)} />
              <EditableNumberField label={t('bayes.sampler.targetAccept')} value={draft.sampler.targetAccept ?? 0.8} min={0} max={1} step={0.01} onChange={(value) => updateNumber('targetAccept', value)} />
              <EditableNumberField label={t('bayes.sampler.maxTreeDepth')} value={draft.sampler.maxTreeDepth ?? 10} min={1} onChange={(value) => updateNumber('maxTreeDepth', value)} />
      </CardContent>
    </Card>
  );
}
