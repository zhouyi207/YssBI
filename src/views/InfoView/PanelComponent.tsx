import React, { useState, useMemo, Suspense } from 'react';
import {
  SectionHeader,
  ModelSummaryGrid,
  PanelFESummaryGrid,
  CoefficientTable,
  CoeffBarChart,
  HypothesisTestBlock,
} from './shared';
import type { PanelSummaryResult, OLSResultData } from './shared/types';

const PanelFormulaBlock = React.lazy(() => import('./PanelFormulaBlock'));

type TabKey = 'fe' | 'fe_time' | 'fe_twoway' | 'lsdv' | 'lsdv_time' | 'fd' | 're';

type ModelType = 'fe' | 're'; // 固定效应 | 随机效应
type EffectType = 'entity' | 'time' | 'twoway'; // 个体 | 时间 | 双向

const MODEL_TYPE_TABS: { key: ModelType; label: string }[] = [
  { key: 'fe', label: 'Fixed Effects' },
  { key: 're', label: 'Random Effects' },
];

const EFFECT_TYPE_TABS: { key: EffectType; label: string }[] = [
  { key: 'entity', label: 'Entity' },
  { key: 'time', label: 'Time' },
  { key: 'twoway', label: 'Two-Way' },
];

// Tab 3 估计方法：根据 (ModelType, EffectType) 映射到 TabKey[]
const METHOD_MAP: Record<ModelType, Record<EffectType, { key: TabKey; label: string }[]>> = {
  fe: {
    entity: [
      { key: 'fe', label: 'Within' },
      { key: 'lsdv', label: 'LSDV' },
      { key: 'fd', label: 'FD' },
    ],
    time: [
      { key: 'fe_time', label: 'Within' },
      { key: 'lsdv_time', label: 'LSDV' },
    ],
    twoway: [{ key: 'fe_twoway', label: 'FE (Two-Way)' }],
  },
  re: {
    entity: [{ key: 're', label: 'RE (Random Effects)' }],
    time: [{ key: 're', label: 'RE' }], // 随机效应通常仅个体，时间/双向复用 RE
    twoway: [{ key: 're', label: 'RE' }],
  },
};

function getDefaultSelections(data: PanelSummaryResult): { model: ModelType; effect: EffectType; method: TabKey } {
  if (data.fe) return { model: 'fe', effect: 'entity', method: 'fe' };
  if (data.lsdv) return { model: 'fe', effect: 'entity', method: 'lsdv' };
  if (data.fd) return { model: 'fe', effect: 'entity', method: 'fd' };
  if (data.fe_time) return { model: 'fe', effect: 'time', method: 'fe_time' };
  if (data.lsdv_time) return { model: 'fe', effect: 'time', method: 'lsdv_time' };
  if (data.fe_twoway) return { model: 'fe', effect: 'twoway', method: 'fe_twoway' };
  if (data.re) return { model: 're', effect: 'entity', method: 're' };
  return { model: 'fe', effect: 'entity', method: 'fe' };
}

export const PanelComponent: React.FC<{ data: PanelSummaryResult }> = ({ data }) => {
  const defaults = getDefaultSelections(data);
  const [modelType, setModelType] = useState<ModelType>(defaults.model);
  const [effectType, setEffectType] = useState<EffectType>(defaults.effect);
  const [activeMethod, setActiveMethod] = useState<TabKey>(defaults.method);

  const methods = useMemo(
    () => METHOD_MAP[modelType][effectType],
    [modelType, effectType]
  );

  // 当切换 model/effect 时，若当前 method 不在新列表中，选第一个
  const currentMethod = useMemo(() => {
    const valid = methods.some((m) => m.key === activeMethod);
    return valid ? activeMethod : methods[0]?.key ?? 'fe';
  }, [activeMethod, methods]);

  const currentData = useMemo(() => {
    const key = currentMethod;
    switch (key) {
      case 'fe':
        return data.fe;
      case 'fe_time':
        return data.fe_time;
      case 'fe_twoway':
        return data.fe_twoway;
      case 'lsdv':
        return data.lsdv;
      case 'lsdv_time':
        return data.lsdv_time;
      case 'fd':
        return data.fd;
      case 're':
        return data.re;
      default:
        return data.fe ?? data.fe_time ?? data.fe_twoway ?? data.lsdv ?? data.lsdv_time ?? data.fd ?? data.re;
    }
  }, [currentMethod, data]);

  const currentError = useMemo(() => {
    return data.errors?.[currentMethod];
  }, [currentMethod, data.errors]);

  const significantCount = useMemo(
    () => (currentData ? currentData.coefficients.filter((c) => c.is_significant).length : 0),
    [currentData]
  );

  const hasCategorical = useMemo(
    () => (currentData ? currentData.coefficients.some((c) => c.category != null) : false),
    [currentData]
  );

  const handleModelChange = (m: ModelType) => {
    setModelType(m);
    const newEffect: EffectType = m === 're' && effectType !== 'entity' ? 'entity' : effectType;
    setEffectType(newEffect);
    const first = METHOD_MAP[m][newEffect][0];
    if (first) setActiveMethod(first.key);
  };

  const handleEffectChange = (e: EffectType) => {
    setEffectType(e);
    const first = METHOD_MAP[modelType][e][0];
    if (first) setActiveMethod(first.key);
  };

  const pillActive = 'bg-[var(--accent-color)]/20 text-[var(--accent-color)] border-[var(--accent-color)]/50';
  const pillInactive = 'text-gray-400 border-gray-700/60 hover:border-gray-600 hover:text-gray-300';

  return (
    <div className="p-6 max-w-[900px] mx-auto">
      {/* Title */}
      <div className="mb-6">
        <h1 className="text-xl font-bold text-white mb-2">{data.title}</h1>
        <span className="text-xs text-gray-500">
          {data.endog_name} &middot; Entity ID + Time ID (cluster by entity)
        </span>
      </div>

      {/* Model selector card: 2 rows */}
      <div className="mb-6 rounded-xl border border-gray-800/60 bg-gray-900/40 overflow-hidden">
        {/* Row 1: Model Type (left) | Effect Type (right) */}
        <div className="flex flex-col sm:flex-row sm:divide-x sm:divide-gray-800/60">
          <div className="flex-1 p-4 flex flex-col items-start">
            <div className="text-[11px] text-gray-500 uppercase tracking-wider mb-2.5 font-medium">
              Model Type
            </div>
            <div className="flex flex-wrap gap-2 justify-start">
              {MODEL_TYPE_TABS.map(({ key, label }) => (
                <button
                  key={key}
                  onClick={() => handleModelChange(key)}
                  className={`px-3.5 py-1.5 text-sm font-medium rounded-lg border transition-all ${
                    modelType === key ? pillActive : pillInactive
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
          <div className="flex-1 p-4 flex flex-col items-end">
            <div className="text-[11px] text-gray-500 uppercase tracking-wider mb-2.5 font-medium">
              Effect Type
            </div>
            <div className="flex flex-wrap gap-2 justify-end">
              {EFFECT_TYPE_TABS.map(({ key, label }) => {
                const disabled = modelType === 're' && (key === 'time' || key === 'twoway');
                return (
                  <button
                    key={key}
                    onClick={() => !disabled && handleEffectChange(key)}
                    disabled={disabled}
                    className={`px-3.5 py-1.5 text-sm font-medium rounded-lg border transition-all ${
                      effectType === key && !disabled ? pillActive : pillInactive
                    } ${disabled ? 'opacity-40 cursor-not-allowed' : ''}`}
                  >
                    {label}
                  </button>
                );
              })}
            </div>
          </div>
        </div>

        {/* Row 2: Estimation Method */}
        <div className="border-t border-gray-800/60 p-4 bg-[#13151a]/50">
          <div className="text-[11px] text-gray-500 uppercase tracking-wider mb-2.5 font-medium">
            Estimation Method
          </div>
          <div className="flex flex-wrap gap-2">
            {methods.map(({ key, label }) => {
              const hasErr = data.errors?.[key];
              const isActive = currentMethod === key;
              return (
                <button
                  key={key}
                  onClick={() => setActiveMethod(key)}
                  className={`px-3.5 py-1.5 text-sm font-medium rounded-lg border transition-all flex items-center gap-1.5 ${
                    isActive ? pillActive : pillInactive
                  }`}
                >
                  {label}
                  {hasErr && (
                    <span className="text-red-400" title={hasErr}>
                      ⚠
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      </div>

      {/* Equation */}
      <SectionHeader
        title="Equation"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4.745 3A23.933 23.933 0 003 12c0 3.183.62 6.22 1.745 9M19.5 3c.967 2.78 1.5 5.817 1.5 9s-.533 6.22-1.5 9M8.25 8.885l1.444-.89a.75.75 0 011.105.402l2.402 7.206a.75.75 0 001.104.401l1.445-.889" />
          </svg>
        }
      />
      <Suspense fallback={<div className="rounded-lg border border-gray-800/50 bg-[#13151a] h-24 animate-pulse" />}>
        <PanelFormulaBlock
          modelType={modelType}
          effectType={effectType}
          method={currentMethod}
        />
      </Suspense>

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
          {(currentMethod === 'fe' || currentMethod === 'fe_time') &&
          currentData?.diagnostic_info?.panel_fe_info ? (
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
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 7h16M4 12h10M4 17h6" />
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
