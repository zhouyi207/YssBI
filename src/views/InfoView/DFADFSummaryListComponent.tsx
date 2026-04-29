import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { formatNum, SignificanceStars } from './shared';
import { DFADFComponent } from './DFADFComponent';
import type {
  DFADFRegRowData,
  DFADFSummaryListResultData,
  DFADFSummaryResultData,
} from './shared/types';

function itemLabel(item: DFADFSummaryResultData): string {
  return `${item.regression} · lags=${item.lags}`;
}

function findRegRow(table: DFADFRegRowData[], name: string): DFADFRegRowData | undefined {
  return table.find((r) => r.variable === name);
}

export const DFADFSummaryListComponent: React.FC<{ data: DFADFSummaryListResultData }> = ({ data }) => {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<DFADFSummaryResultData | null>(null);

  return (
    <div className="relative">
      {/* 主列表 */}
      <div className="p-6 max-w-[1100px] mx-auto">
        <div className="mb-6">
          <h1 className="text-xl font-bold text-white mb-2">{data.title}</h1>
          <div className="text-xs text-gray-500">
            Variable: {data.var_name} · {data.items.length} combinations
          </div>
        </div>
        <div className="rounded-lg border border-gray-800/50 overflow-hidden">
          <table className="w-full text-xs">
            <thead>
              <tr className="bg-[#1a1d23]">
                <th className="text-left px-4 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Variable</th>
                <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Lags</th>
                <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Z(t)</th>
                <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">P&gt;|t|</th>
                <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">const (p)</th>
                <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">trend (p)</th>
              </tr>
            </thead>
            <tbody>
              {data.items.map((item, idx) => {
                const reject = item.test_statistic < item.critical_value_5pct;
                const isActive = selected === item;
                const cons = findRegRow(item.regression_table, 'const');
                const trend = findRegRow(item.regression_table, 'trend');
                return (
                  <tr
                    key={idx}
                    onClick={() => setSelected(item)}
                    className={`
                      border-t border-gray-800/30 transition-colors cursor-pointer hover:bg-[#1e2128]
                      ${idx % 2 === 0 ? 'bg-[#13151a]' : 'bg-[#15171d]'}
                      ${isActive ? 'ring-2 ring-inset ring-[var(--accent-color)]' : ''}
                    `}
                  >
                    <td className="px-4 py-2.5">
                      <div className="flex items-center gap-2">
                        <div className={`w-1.5 h-1.5 rounded-full ${reject ? 'bg-emerald-400' : 'bg-gray-600'}`} />
                        <span className={`font-mono font-medium ${reject ? 'text-white' : 'text-gray-400'}`}>
                          {item.regression}
                        </span>
                      </div>
                    </td>
                    <td className="text-right px-3 py-2.5 font-mono text-gray-300">{item.lags}</td>
                    <td className="text-right px-3 py-2.5 font-mono text-white">{formatNum(item.test_statistic)}</td>
                    <td className="text-right px-3 py-2.5 font-mono">
                      <span className={reject ? 'text-emerald-400' : 'text-gray-500'}>
                        {formatNum(item.p_value, 3)}
                      </span>
                      <SignificanceStars pValue={item.p_value} />
                    </td>
                    <td className="text-right px-3 py-2.5 font-mono">
                      {cons ? (
                        <>
                          <span className={cons.p_value < 0.05 ? 'text-emerald-400' : 'text-gray-500'}>
                            {formatNum(cons.p_value, 3)}
                          </span>
                          <SignificanceStars pValue={cons.p_value} />
                        </>
                      ) : (
                        <span className="text-gray-600">—</span>
                      )}
                    </td>
                    <td className="text-right px-3 py-2.5 font-mono">
                      {trend ? (
                        <>
                          <span className={trend.p_value < 0.05 ? 'text-emerald-400' : 'text-gray-500'}>
                            {formatNum(trend.p_value, 3)}
                          </span>
                          <SignificanceStars pValue={trend.p_value} />
                        </>
                      ) : (
                        <span className="text-gray-600">—</span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        <div className="flex items-center gap-4 mt-2 text-[10px] text-gray-600 px-1">
          <span>Significance: <span className="text-yellow-400">***</span> p&lt;0.001, <span className="text-yellow-400">**</span> p&lt;0.01, <span className="text-yellow-400">*</span> p&lt;0.05, <span className="text-gray-500">.</span> p&lt;0.1</span>
        </div>
      </div>

      {/* Drawer：从右侧滑入 */}
      {selected && (
        <>
          <div
            className="fixed left-0 right-0 bottom-0 z-40 bg-black/40 transition-opacity"
            style={{ top: '2.5rem' }}
            onClick={() => setSelected(null)}
            aria-hidden="true"
          />
          <div
            className="fixed right-0 bottom-0 w-[min(90vw,900px)] bg-[var(--workbench-bg)] border-l border-gray-800/50 z-50 shadow-2xl animate-slide-in flex flex-col min-h-0"
            style={{ top: '2.5rem' }}
          >
            <div className="bg-[var(--workbench-bg)] border-b border-gray-800/50 px-4 py-3 flex items-center justify-between z-10 shrink-0">
              <span className="text-sm font-medium text-gray-400">{itemLabel(selected)}</span>
              <button
                type="button"
                onClick={() => setSelected(null)}
                className="p-1.5 rounded text-gray-500 hover:text-white hover:bg-gray-800/50 transition-colors"
                title={t('common.close')}
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
            <OverlayScrollbar className="flex-1">
              <DFADFComponent data={selected} />
            </OverlayScrollbar>
          </div>
        </>
      )}
    </div>
  );
};
