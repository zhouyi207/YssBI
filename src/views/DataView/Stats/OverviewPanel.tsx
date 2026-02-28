import React from 'react';
import type { DatasetOverview } from '@/features/core/dataStore/datasetOverviewStore';

interface OverviewPanelProps {
  overview: DatasetOverview;
  statsLoading: boolean;
}

const StatRow: React.FC<{ label: string; value: string | number }> = ({ label, value }) => (
  <>
    <div className="text-gray-500">{label}</div>
    <div className="font-mono text-gray-400 text-right">{value}</div>
  </>
);

const Section: React.FC<{ title: string; icon: React.ReactNode; children: React.ReactNode }> = ({ title, icon, children }) => (
  <div className="flex-1 min-w-0 rounded border border-gray-800 bg-[var(--workbench-bg)]/50 overflow-hidden">
    <div className="flex items-center gap-1.5 px-2.5 py-1 border-b border-gray-800/50">
      {icon}
      <span className="text-[9px] font-bold uppercase tracking-widest text-gray-500">{title}</span>
    </div>
    <div className="px-2.5 py-2">{children}</div>
  </div>
);

const fmtMemory = (bytes: number): string => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
};

const fmtPercent = (v: number): string => `${(v * 100).toFixed(2)}%`;

export const OverviewPanel: React.FC<OverviewPanelProps> = ({ overview, statsLoading }) => {
  const { sizeShape, schemaOverview, dataCompleteness } = overview;

  return (
    <div className="shrink-0 border-b border-gray-800">
      <div className="h-8 flex items-center gap-2 px-3 border-b border-gray-800 shrink-0">
        <svg className="w-3.5 h-3.5 text-[var(--accent-color)]" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
          <rect x="2" y="2" width="12" height="12" rx="1" />
          <line x1="2" y1="6" x2="14" y2="6" />
          <line x1="2" y1="10" x2="14" y2="10" />
          <line x1="6" y1="2" x2="6" y2="14" />
        </svg>
        <span className="text-[11px] font-bold uppercase tracking-widest text-gray-500">Overview</span>
        {statsLoading && <span className="text-[9px] text-[var(--accent-color)] animate-pulse ml-auto">loading…</span>}
      </div>
      <div className="p-2.5 flex gap-2">
        <Section
          title="Size & Shape"
          icon={<svg className="w-2.5 h-2.5 text-[var(--accent-color)]/70" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5"><rect x="1" y="1" width="14" height="14" rx="1" /><line x1="8" y1="1" x2="8" y2="15" /><line x1="1" y1="8" x2="15" y2="8" /></svg>}
        >
          <div className="grid grid-cols-2 gap-x-1.5 gap-y-1 text-[9px]">
            <StatRow label="n_rows" value={sizeShape.nRows.toLocaleString()} />
            <StatRow label="n_columns" value={sizeShape.nColumns} />
            <StatRow label="memory" value={fmtMemory(sizeShape.memorySize)} />
            <StatRow label="duplicated" value={sizeShape.duplicatedRows.toLocaleString()} />
          </div>
        </Section>
        <Section
          title="Schema"
          icon={<svg className="w-2.5 h-2.5 text-[var(--accent-color)]/70" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M4 2v12M12 2v12M1 5h14M1 11h14" /></svg>}
        >
          <div className="grid grid-cols-2 gap-x-1.5 gap-y-1 text-[9px]">
            <StatRow label="numeric" value={schemaOverview.numericCols} />
            <StatRow label="categorical" value={schemaOverview.categoricalCols} />
            <StatRow label="string" value={schemaOverview.stringCols} />
            <StatRow label="datetime" value={schemaOverview.datetimeCols} />
            <StatRow label="bool" value={schemaOverview.boolCols} />
          </div>
        </Section>
        <Section
          title="Completeness"
          icon={<svg className="w-2.5 h-2.5 text-[var(--accent-color)]/70" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5"><circle cx="8" cy="8" r="6" /><path d="M8 5v4l2.5 1.5" /></svg>}
        >
          <div className="grid grid-cols-2 gap-x-1.5 gap-y-1 text-[9px]">
            <StatRow label="nulls" value={dataCompleteness.totalNulls.toLocaleString()} />
            <StatRow label="null_ratio" value={fmtPercent(dataCompleteness.nullRatio)} />
            <StatRow label="null_cols" value={dataCompleteness.colsWithNulls} />
            <StatRow label="null_rows" value={dataCompleteness.rowsWithNulls.toLocaleString()} />
          </div>
        </Section>
      </div>
    </div>
  );
};
