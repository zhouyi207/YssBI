import React, { useState, useMemo } from 'react';
import {
  SectionHeader,
  ModelSummaryGrid,
  PanelFESummaryGrid,
  CoefficientTable,
  CoeffBarChart,
  HypothesisTestBlock,
} from './shared';
import type { PanelSummaryResult, OLSResultData } from './shared/types';

type TabKey = 'fe' | 'lsdv' | 'fd' | 're';

const TABS: { key: TabKey; label: string }[] = [
  { key: 'fe', label: 'FE (Within)' },
  { key: 'lsdv', label: 'LSDV' },
  { key: 'fd', label: 'First Difference (FD)' },
  { key: 're', label: 'Random Effects (RE)' },
];

export const PanelComponent: React.FC<{ data: PanelSummaryResult }> = ({ data }) => {
  const defaultTab: TabKey = data.fe ? 'fe' : data.lsdv ? 'lsdv' : data.fd ? 'fd' : 're';
  const [activeTab, setActiveTab] = useState<TabKey>(defaultTab);

  const currentData = useMemo(() => {
    switch (activeTab) {
      case 'fe':
        return data.fe;
      case 'lsdv':
        return data.lsdv;
      case 'fd':
        return data.fd;
      case 're':
        return data.re;
      default:
        return data.fe ?? data.lsdv ?? data.fd ?? data.re;
    }
  }, [activeTab, data]);

  const currentError = useMemo(() => {
    return data.errors?.[activeTab];
  }, [activeTab, data.errors]);

  const significantCount = useMemo(
    () => (currentData ? currentData.coefficients.filter((c) => c.is_significant).length : 0),
    [currentData]
  );

  const hasCategorical = useMemo(
    () => (currentData ? currentData.coefficients.some((c) => c.category != null) : false),
    [currentData]
  );

  return (
    <div className="p-6 max-w-[900px] mx-auto">
      {/* Title */}
      <div className="mb-6">
        <h1 className="text-xl font-bold text-white mb-2">{data.title}</h1>
        <span className="text-xs text-gray-500">
          {data.endog_name} &middot; Entity ID + Time ID (cluster by entity)
        </span>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 mb-6 border-b border-gray-800/50">
        {TABS.map(({ key, label }) => {
          const hasResult = key === 'fe' ? data.fe : key === 'lsdv' ? data.lsdv : key === 'fd' ? data.fd : data.re;
          const hasErr = data.errors?.[key];
          const isActive = activeTab === key;
          return (
            <button
              key={key}
              onClick={() => setActiveTab(key)}
              className={`px-4 py-2.5 text-sm font-medium rounded-t-md transition-colors ${
                isActive
                  ? 'bg-[var(--sidebar-bg)] text-white border-b-2 border-[var(--accent-color)] -mb-px'
                  : 'text-gray-400 hover:text-white hover:bg-gray-800/50'
              }`}
            >
              {label}
              {hasErr && (
                <span className="ml-1.5 text-red-400" title={hasErr}>
                  ⚠
                </span>
              )}
            </button>
          );
        })}
      </div>

      {/* Error state */}
      {currentError && (
        <div className="mb-6 rounded-lg border border-red-500/30 bg-red-500/10 p-4">
          <div className="flex items-start gap-2">
            <svg
              className="w-5 h-5 text-red-400 shrink-0 mt-0.5"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z"
              />
            </svg>
            <div>
              <div className="font-medium text-red-400">Model failed</div>
              <div className="text-sm text-gray-300 mt-1">{currentError}</div>
            </div>
          </div>
        </div>
      )}

      {/* Result content */}
      {currentData && !currentError && (
        <>
          {/* Model Summary */}
          <SectionHeader
            title="Model Summary"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
                />
              </svg>
            }
          />
          {activeTab === 'fe' && currentData?.diagnostic_info?.panel_fe_info ? (
            <PanelFESummaryGrid
              info={currentData.model_basic_info}
              panelFe={currentData.diagnostic_info.panel_fe_info}
            />
          ) : (
            <ModelSummaryGrid info={currentData.model_basic_info} />
          )}

          {/* Coefficients */}
          <SectionHeader
            title={`Coefficients (${significantCount}/${currentData.coefficients.length} significant)`}
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M4 7h16M4 12h10M4 17h6"
                />
              </svg>
            }
          />
          <CoefficientTable
            coefficients={currentData.coefficients}
            hasCategorical={hasCategorical}
          />

          {/* Hypothesis Test */}
          <HypothesisTestBlock data={currentData as OLSResultData} />

          {/* Coefficient Bar */}
          <SectionHeader
            title="Coefficient Magnitude"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M16 8v8m-4-5v5m-4-2v2m-2 4h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"
                />
              </svg>
            }
          />
          <CoeffBarChart coefficients={currentData.coefficients} />
        </>
      )}
    </div>
  );
};
