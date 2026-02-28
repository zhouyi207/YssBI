import React from 'react';
import { VscSymbolNumeric } from 'react-icons/vsc';
import type { ColumnMeta } from '@/features/application/dataView';
import { COLUMN_TYPE_OPTIONS } from '@/features/application/dataView';
import type { ColumnStats, NumericColumnStats, StringColumnStats } from '@/features/core/dataStore/columnStatsStore';
import type { ColumnDistribution, NumericDistribution, StringDistribution } from '@/features/core/dataStore/columnDistributionStore';
import Histogram from '@/views/PlotView/Histogram';
import BarChart from '@/views/PlotView/BarChart';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { Select } from '@/shared/ui';

interface ColumnStatsPanelProps {
  columns: ColumnMeta[];
  columnStatsMap?: Record<string, ColumnStats>;
  columnDistMap?: Record<string, ColumnDistribution>;
  statsLoading: boolean;
  onCastColumn?: (colName: string, newDtype: string) => void;
}

const fmtNum = (v: number | null | undefined, digits = 4) =>
  v == null ? '—' : Number.isInteger(v) ? String(v) : v.toFixed(digits);

export const ColumnStatsPanel: React.FC<ColumnStatsPanelProps> = ({
  columns, columnStatsMap, columnDistMap, statsLoading, onCastColumn,
}) => (
  <>
    <div className="h-7 flex items-center gap-2 px-3 border-b border-gray-800 shrink-0">
      <VscSymbolNumeric className="text-[var(--accent-color)]" size={13} />
      <span className="text-[11px] font-bold uppercase tracking-widest text-gray-500">Column Stats</span>
      {statsLoading && <span className="text-[9px] text-[var(--accent-color)] animate-pulse ml-auto">loading…</span>}
    </div>
    <OverlayScrollbar className="flex-1 min-h-0">
      <div className="p-2.5 space-y-3">
        {columns.map((col, i) => {
          const stat: ColumnStats | undefined = columnStatsMap?.[col.name];
          const dist: ColumnDistribution | undefined = columnDistMap?.[col.name];
          return (
            <div key={i} className="rounded border border-gray-800 bg-[var(--workbench-bg)]/50 p-2.5 space-y-1.5">
              <div className="grid grid-cols-[minmax(0,1fr)_80px] gap-2 items-center pb-1.5 border-b border-gray-800/50">
                <span className="text-[11px] font-bold text-gray-300 truncate min-w-0">{col.name}</span>
                {onCastColumn ? (
                  <div className="w-full min-w-0">
                    <Select
                      value={col.type}
                      onChange={(v) => { if (v !== col.type) onCastColumn(col.name, v); }}
                      options={(() => {
                        const opts = COLUMN_TYPE_OPTIONS.map(o => ({ label: o.label, value: o.value }));
                        if (col.type && !opts.some(o => o.value === col.type)) {
                          opts.unshift({ label: col.type, value: col.type });
                        }
                        return opts;
                      })()}
                      className="text-[9px] h-5 font-mono !w-full"
                    />
                  </div>
                ) : (
                  <span className="text-[9px] font-mono text-[var(--accent-color)]/70 shrink-0">{col.type}</span>
                )}
              </div>
              {!stat ? (
                <div className="text-[10px] text-gray-600 italic py-1">{statsLoading ? 'computing…' : 'no data'}</div>
              ) : (
                <div className="flex gap-3 items-stretch">
                  <div className="w-36 shrink-0">
                    {stat.kind === 'string' ? (
                      <div className="grid grid-cols-2 gap-x-2 gap-y-1 text-[10px]">
                        <div className="text-gray-500">count</div><div className="font-mono text-gray-400 text-right">{stat.count}</div>
                        <div className="text-gray-500">null_count</div><div className="font-mono text-gray-400 text-right">{stat.nullCount}</div>
                        <div className="text-gray-500">empty_count</div><div className="font-mono text-gray-400 text-right">{(stat as StringColumnStats).emptyCount}</div>
                        <div className="text-gray-500">valid_ratio</div><div className="font-mono text-gray-400 text-right">{fmtNum((stat as StringColumnStats).validRatio, 2)}</div>
                        <div className="text-gray-500">unique</div><div className="font-mono text-gray-400 text-right">{(stat as StringColumnStats).unique}</div>
                        <div className="text-gray-500">mode</div><div className="font-mono text-gray-400 text-right truncate" title={(stat as StringColumnStats).mode ?? ''}>{(stat as StringColumnStats).mode ?? '—'}</div>
                        <div className="text-gray-500">mode_count</div><div className="font-mono text-gray-400 text-right">{(stat as StringColumnStats).modeCount}</div>
                      </div>
                    ) : (
                      <div className="grid grid-cols-2 gap-x-2 gap-y-1 text-[10px]">
                        <div className="text-gray-500">count</div><div className="font-mono text-gray-400 text-right">{stat.count}</div>
                        <div className="text-gray-500">null_count</div><div className="font-mono text-gray-400 text-right">{stat.nullCount}</div>
                        <div className="text-gray-500">min</div><div className="font-mono text-gray-400 text-right truncate">{fmtNum((stat as NumericColumnStats).min)}</div>
                        <div className="text-gray-500">max</div><div className="font-mono text-gray-400 text-right truncate">{fmtNum((stat as NumericColumnStats).max)}</div>
                        <div className="text-gray-500">mean</div><div className="font-mono text-gray-400 text-right">{fmtNum((stat as NumericColumnStats).mean)}</div>
                        <div className="text-gray-500">median</div><div className="font-mono text-gray-400 text-right">{fmtNum((stat as NumericColumnStats).median)}</div>
                        <div className="text-gray-500">std</div><div className="font-mono text-gray-400 text-right">{fmtNum((stat as NumericColumnStats).std)}</div>
                        <div className="text-gray-500">variance</div><div className="font-mono text-gray-400 text-right">{fmtNum((stat as NumericColumnStats).variance)}</div>
                      </div>
                    )}
                  </div>
                  <div className="flex-1 min-w-0 min-h-0">
                    {dist ? (
                      dist.kind === 'numeric' ? (
                        <Histogram data={(dist as NumericDistribution).bins} compact />
                      ) : (
                        <BarChart data={(dist as StringDistribution).categories} horizontal compact />
                      )
                    ) : (
                      <div className="h-full flex items-center justify-center text-[10px] text-gray-600 italic border border-gray-800/30 rounded">
                        {statsLoading ? 'loading…' : '—'}
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </OverlayScrollbar>
  </>
);
