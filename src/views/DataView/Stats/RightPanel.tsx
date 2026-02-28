import React from 'react';
import type { ColumnMeta } from '@/features/application/dataView';
import type { ColumnStats } from '@/features/core/dataStore/columnStatsStore';
import type { ColumnDistribution } from '@/features/core/dataStore/columnDistributionStore';
import type { DatasetOverview } from '@/features/core/dataStore/datasetOverviewStore';
import { OverviewPanel } from './OverviewPanel';
import { ColumnStatsPanel } from './ColumnStatsPanel';

interface RightPanelProps {
  columns: ColumnMeta[];
  overview?: DatasetOverview;
  columnStatsMap?: Record<string, ColumnStats>;
  columnDistMap?: Record<string, ColumnDistribution>;
  statsLoading: boolean;
  onCastColumn?: (colName: string, newDtype: string) => void;
}

export const RightPanel: React.FC<RightPanelProps> = ({
  columns, overview, columnStatsMap, columnDistMap, statsLoading, onCastColumn,
}) => (
  <div className="w-[520px] shrink-0 flex flex-col border-l border-gray-800 bg-[var(--sidebar-bg)] h-full overflow-hidden">
    {overview && <OverviewPanel overview={overview} statsLoading={statsLoading} />}
    <ColumnStatsPanel
      columns={columns}
      columnStatsMap={columnStatsMap}
      columnDistMap={columnDistMap}
      statsLoading={statsLoading}
      onCastColumn={onCastColumn}
    />
  </div>
);
