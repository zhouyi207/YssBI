import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { VscRefresh } from 'react-icons/vsc';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { useGraphTraceDetails } from '@/features/application/observability/useGraphTraceDetails';
import type { TraceRecordDto, TraceValueDto } from '@/shared/types/dto/trace';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { DetailCollapsibleSection } from '../shared/DetailCollapsibleSection';
import { detailEmptyHintClass } from '../shared/detailStyles';
import { DetailBadge, DetailText } from '../shared/DetailText';

interface GraphTraceDetailsProps {
  graphPath: string;
}

export function GraphTraceDetails({ graphPath }: GraphTraceDetailsProps) {
  const { t } = useTranslation();
  const trace = useGraphTraceDetails(graphPath);
  const runIds = useMemo(
    () => Array.from(new Set(trace.graphTraces.flatMap((record) =>
      record.correlation.runId === null ? [] : [record.correlation.runId],
    ))),
    [trace.graphTraces],
  );
  const records = trace.selectedRunId === null ? trace.graphTraces : trace.runTrace;
  const loading = trace.selectedRunId === null ? trace.graphLoading : trace.runLoading;
  const error = trace.selectedRunId === null ? trace.graphError : trace.runError;

  return (
    <DetailCollapsibleSection title={t('detail.trace.title')}>
      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between gap-2">
          <DetailText tone="smallMuted">{t('detail.trace.runs')}</DetailText>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 gap-1.5 rounded-md px-2 text-xs"
            disabled={trace.graphLoading}
            onClick={() => void trace.refresh()}
          >
            <VscRefresh className="size-3.5" />
            {t('detail.trace.refresh')}
          </Button>
        </div>

        <div className="flex flex-wrap gap-1.5">
          <RunButton
            selected={trace.selectedRunId === null}
            onClick={() => void trace.selectRun(null)}
          >
            {t('detail.trace.allRuns')}
          </RunButton>
          {runIds.map((runId) => (
            <RunButton
              key={runId}
              selected={trace.selectedRunId === runId}
              onClick={() => void trace.selectRun(runId)}
            >
              {t('detail.trace.run')} {runId}
            </RunButton>
          ))}
        </div>

        <TraceContent
          records={records}
          loading={loading}
          error={error !== null}
          runSelected={trace.selectedRunId !== null}
          runNotFound={trace.selectedRunNotFound}
        />
      </div>
    </DetailCollapsibleSection>
  );
}

function RunButton({
  children,
  selected,
  onClick,
}: {
  children: React.ReactNode;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <Button
      type="button"
      variant={selected ? 'secondary' : 'outline'}
      size="sm"
      className="h-7 rounded-md px-2 text-xs"
      aria-pressed={selected}
      onClick={onClick}
    >
      {children}
    </Button>
  );
}

function TraceContent({
  records,
  loading,
  error,
  runSelected,
  runNotFound,
}: {
  records: TraceRecordDto[];
  loading: boolean;
  error: boolean;
  runSelected: boolean;
  runNotFound: boolean;
}) {
  const { t } = useTranslation();

  if (loading) return <TraceState>{t('detail.trace.loading')}</TraceState>;
  if (runNotFound) return <TraceState>{t('detail.trace.runNotFound')}</TraceState>;
  if (error) {
    return (
      <TraceState>
        {t(runSelected ? 'detail.trace.runError' : 'detail.trace.graphError')}
      </TraceState>
    );
  }
  if (records.length === 0) return <TraceState>{t('detail.trace.empty')}</TraceState>;

  return (
    <OverlayScrollbar
      className="max-h-80 rounded-md border border-border/60 bg-background/30"
      direction="vertical"
    >
      <div className="space-y-2 p-2">
        {records.map((record) => (
          <TraceRecord key={record.sequence} record={record} />
        ))}
      </div>
    </OverlayScrollbar>
  );
}

function TraceState({ children }: { children: React.ReactNode }) {
  return <div className={detailEmptyHintClass}>{children}</div>;
}

function TraceRecord({ record }: { record: TraceRecordDto }) {
  const { t } = useTranslation();
  const correlation = record.correlation;

  return (
    <Card className="gap-0 rounded-md bg-card/80 py-0 shadow-xs">
      <CardHeader className="flex-row items-center justify-between gap-2 px-2.5 py-2">
        <div className="flex min-w-0 items-center gap-1.5">
          <DetailText tone="mono">#{record.sequence}</DetailText>
          <DetailBadge>{record.kind}</DetailBadge>
        </div>
        <DetailBadge>{record.status}</DetailBadge>
      </CardHeader>
      <CardContent className="space-y-2 border-t border-border/60 px-2.5 py-2">
        <TraceGroup title={t('detail.trace.correlation')}>
          <TraceValue label={t('detail.trace.projectSession')} value={correlation.projectSessionId} />
          <TraceValue label={t('detail.trace.graphPath')} value={correlation.graphPath} />
          <TraceValue label={t('detail.trace.graphRevision')} value={correlation.graphRevision} />
          <TraceValue label={t('detail.trace.registryFingerprint')} value={correlation.registryFingerprint} />
          <TraceValue label={t('detail.trace.compileId')} value={correlation.compileId} />
          <TraceValue label={t('detail.trace.runId')} value={correlation.runId} />
          <TraceValue label={t('detail.trace.nodeId')} value={correlation.nodeId} />
          <TraceValue label={t('detail.trace.nodeTypeId')} value={correlation.nodeTypeId} />
          <TraceValue label={t('detail.trace.parentCall')} value={correlation.parentCall} />
          {Object.entries(correlation.resourceVersions).map(([name, version]) => (
            <TraceValue
              key={name}
              label={`${t('detail.trace.resourceVersions')} · ${name}`}
              value={version}
            />
          ))}
        </TraceGroup>
        {Object.keys(record.fields).length > 0 && (
          <TraceGroup title={t('detail.trace.publicFields')}>
            {Object.entries(record.fields).map(([name, value]) => (
              <TraceField key={name} name={name} value={value} />
            ))}
          </TraceGroup>
        )}
      </CardContent>
    </Card>
  );
}

function TraceGroup({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="space-y-1">
      <DetailText as="div" tone="smallMuted" className="font-semibold uppercase tracking-wide">
        {title}
      </DetailText>
      <dl className="space-y-1">{children}</dl>
    </div>
  );
}

function TraceValue({ label, value }: { label: string; value: string | null }) {
  const { t } = useTranslation();
  return (
    <div className="grid min-w-0 grid-cols-[minmax(0,2fr)_minmax(0,3fr)] gap-2">
      <DetailText as="dt" tone="smallMuted" className="truncate" title={label}>
        {label}
      </DetailText>
      <DetailText as="dd" tone="mono" className="break-all text-right">
        {value ?? t('detail.trace.none')}
      </DetailText>
    </div>
  );
}

function TraceField({ name, value }: { name: string; value: TraceValueDto }) {
  const { t } = useTranslation();
  const rendered = value.type === 'redacted' ? t('detail.trace.redacted') : String(value.value);
  return <TraceValue label={name} value={rendered} />;
}
