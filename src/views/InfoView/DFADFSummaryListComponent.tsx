import { useState, type FC } from 'react';
import { useTranslation } from 'react-i18next';
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { ReportLayout, formatNum, SignificanceStars } from './shared';
import {
  InfoStatsTable,
  infoStatsCellClass,
  infoStatsCellRightClass,
  infoStatsHeadClass,
  infoStatsHeadCompactClass,
  infoStatsRowEvenClass,
  infoStatsRowOddClass,
} from './shared/InfoStatsTable';
import { DFADFComponent } from './DFADFComponent';
import type {
  DFADFRegRowData,
  DFADFSummaryListResultData,
  DFADFSummaryResultData,
} from '@/shared/types/report';

function itemLabel(item: DFADFSummaryResultData): string {
  return `${item.regression} · lags=${item.lags}`;
}

function findRegRow(table: DFADFRegRowData[], name: string): DFADFRegRowData | undefined {
  return table.find((r) => r.variable === name);
}

export const DFADFSummaryListComponent: FC<{ data: DFADFSummaryListResultData }> = ({ data }) => {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<DFADFSummaryResultData | null>(null);

  return (
    <div className="relative">
      <ReportLayout
        title={data.title}
        size="extraWide"
        badges={
          <span className="text-xs text-muted-foreground">
            Variable: {data.var_name} · {data.items.length} combinations
          </span>
        }
      >
        <InfoStatsTable>
          <TableHeader>
            <TableRow className="border-0 hover:bg-transparent">
              <TableHead className={infoStatsHeadClass}>Variable</TableHead>
              <TableHead className={infoStatsHeadCompactClass}>Lags</TableHead>
              <TableHead className={infoStatsHeadCompactClass}>Z(t)</TableHead>
              <TableHead className={infoStatsHeadCompactClass}>P&gt;|t|</TableHead>
              <TableHead className={infoStatsHeadCompactClass}>const (p)</TableHead>
              <TableHead className={infoStatsHeadCompactClass}>trend (p)</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {data.items.map((item, idx) => {
              const reject = item.test_statistic < item.critical_value_5pct;
              const isActive = selected === item;
              const cons = findRegRow(item.regression_table, 'const');
              const trend = findRegRow(item.regression_table, 'trend');
              return (
                <TableRow
                  key={idx}
                  onClick={() => setSelected(item)}
                  className={`cursor-pointer transition-colors hover:bg-muted ${
                    idx % 2 === 0 ? infoStatsRowEvenClass : infoStatsRowOddClass
                  } ${isActive ? 'ring-2 ring-inset ring-[var(--accent-color)]' : ''}`}
                >
                  <TableCell className={infoStatsCellClass}>
                    <div className="flex items-center gap-2">
                      <div className={`h-1.5 w-1.5 rounded-full ${reject ? 'bg-emerald-400' : 'bg-muted-foreground/40'}`} />
                      <span className={`font-mono font-medium ${reject ? 'text-foreground' : 'text-muted-foreground'}`}>
                        {item.regression}
                      </span>
                    </div>
                  </TableCell>
                  <TableCell className={`${infoStatsCellRightClass} text-foreground`}>{item.lags}</TableCell>
                  <TableCell className={`${infoStatsCellRightClass} text-foreground`}>{formatNum(item.test_statistic)}</TableCell>
                  <TableCell className={infoStatsCellRightClass}>
                    <span className={reject ? 'text-emerald-400' : 'text-muted-foreground'}>{formatNum(item.p_value, 3)}</span>
                    <SignificanceStars pValue={item.p_value} />
                  </TableCell>
                  <TableCell className={infoStatsCellRightClass}>
                    {cons ? (
                      <>
                        <span className={cons.p_value < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}>
                          {formatNum(cons.p_value, 3)}
                        </span>
                        <SignificanceStars pValue={cons.p_value} />
                      </>
                    ) : (
                      <span className="text-muted-foreground">—</span>
                    )}
                  </TableCell>
                  <TableCell className={infoStatsCellRightClass}>
                    {trend ? (
                      <>
                        <span className={trend.p_value < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}>
                          {formatNum(trend.p_value, 3)}
                        </span>
                        <SignificanceStars pValue={trend.p_value} />
                      </>
                    ) : (
                      <span className="text-muted-foreground">—</span>
                    )}
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </InfoStatsTable>
        <div className="mt-2 flex items-center gap-4 px-1 text-[10px] text-muted-foreground">
          <span>
            Significance: <span className="text-yellow-400">***</span> p&lt;0.001, <span className="text-yellow-400">**</span> p&lt;0.01,{' '}
            <span className="text-yellow-400">*</span> p&lt;0.05, <span className="text-muted-foreground">.</span> p&lt;0.1
          </span>
        </div>
      </ReportLayout>

      {selected && (
        <>
          <div
            className="fixed bottom-0 left-0 right-0 z-40 bg-black/40 transition-opacity"
            style={{ top: '2.5rem' }}
            onClick={() => setSelected(null)}
            aria-hidden="true"
          />
          <div
            className="fixed bottom-0 right-0 z-50 flex min-h-0 w-[min(90vw,900px)] animate-slide-in flex-col border-l border-border bg-[var(--workbench-bg)] shadow-2xl"
            style={{ top: '2.5rem' }}
          >
            <div className="z-10 flex shrink-0 items-center justify-between border-b border-border bg-[var(--workbench-bg)] px-4 py-3">
              <span className="text-sm font-medium text-muted-foreground">{itemLabel(selected)}</span>
              <Button type="button" variant="ghost" size="icon-sm" onClick={() => setSelected(null)} aria-label={t('common.close')}>
                <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </Button>
            </div>
            <ScrollArea className="flex-1">
              <DFADFComponent data={selected} />
            </ScrollArea>
          </div>
        </>
      )}
    </div>
  );
};
