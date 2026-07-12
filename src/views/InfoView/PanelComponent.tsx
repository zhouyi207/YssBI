import React, { useState, useMemo } from 'react';
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import {
  ReportLayout,
  ReportLazyBoundary,
  ReportSection,
  LazyPanelFormulaBlock,
  ModelSummaryGrid,
  PanelFESummaryGrid,
  PanelBESummaryGrid,
  PanelRESummaryGrid,
  PanelSelectionTestsBlock,
  PanelMLEIterationBlock,
  CoefficientsBlock,
  HypothesisTestBlock,
  OmittedVariablesAlert,
} from './shared';
import type { PanelSummaryResult, OLSResultData } from '@/shared/types/report';
import type { PanelEffectType as EffectType, PanelMethod as TabKey, PanelModelType as ModelType } from './PanelFormulaBlock';

const MODEL_TYPE_TABS: { key: ModelType; label: string }[] = [
  { key: 'mixed', label: 'Mixed Regression' },
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
  mixed: {
    none: [{ key: 'mixed_ols', label: 'OLS' }],
    entity: [{ key: 'mixed_ols', label: 'OLS' }],
    time: [{ key: 'mixed_ols', label: 'OLS' }],
    twoway: [{ key: 'mixed_ols', label: 'OLS' }],
  },
  fe: {
    none: [],
    entity: [
      { key: 'fe', label: 'Within' },
      { key: 'lsdv', label: 'LSDV' },
      { key: 'fd', label: 'FD' },
    ],
    time: [
      { key: 'fe_time', label: 'Within' },
      { key: 'lsdv_time', label: 'LSDV' },
    ],
    twoway: [
      { key: 'fe_twoway', label: 'Within' },
      { key: 'lsdv_twoway', label: 'LSDV' },
    ],
  },
  re: {
    none: [],
    entity: [
      { key: 're_fgls', label: 'FGLS' },
      { key: 're_mle', label: 'MLE' },
      { key: 're_be', label: 'BE' },
    ],
    time: [
      { key: 're_fgls_time', label: 'FGLS' },
      { key: 're_mle_time', label: 'MLE' },
      { key: 're_be_time', label: 'BE' },
    ],
    twoway: [
      { key: 're_fgls_twoway', label: 'FGLS' },
      { key: 're_mle_twoway', label: 'MLE' },
    ],
  },
};

function getDefaultSelections(data: PanelSummaryResult): { model: ModelType; effect: EffectType; method: TabKey } {
  if (data.mixed_ols) return { model: 'mixed', effect: 'none', method: 'mixed_ols' };
  if (data.fe) return { model: 'fe', effect: 'entity', method: 'fe' };
  if (data.lsdv) return { model: 'fe', effect: 'entity', method: 'lsdv' };
  if (data.fd) return { model: 'fe', effect: 'entity', method: 'fd' };
  if (data.fe_time) return { model: 'fe', effect: 'time', method: 'fe_time' };
  if (data.lsdv_time) return { model: 'fe', effect: 'time', method: 'lsdv_time' };
  if (data.fe_twoway) return { model: 'fe', effect: 'twoway', method: 'fe_twoway' };
  if (data.lsdv_twoway) return { model: 'fe', effect: 'twoway', method: 'lsdv_twoway' };
  if (data.re_fgls) return { model: 're', effect: 'entity', method: 're_fgls' };
  if (data.re_mle) return { model: 're', effect: 'entity', method: 're_mle' };
  if (data.re_be) return { model: 're', effect: 'entity', method: 're_be' };
  if (data.re_fgls_time) return { model: 're', effect: 'time', method: 're_fgls_time' };
  if (data.re_mle_time) return { model: 're', effect: 'time', method: 're_mle_time' };
  if (data.re_be_time) return { model: 're', effect: 'time', method: 're_be_time' };
  if (data.re_fgls_twoway) return { model: 're', effect: 'twoway', method: 're_fgls_twoway' };
  if (data.re_mle_twoway) return { model: 're', effect: 'twoway', method: 're_mle_twoway' };
  return { model: 'mixed', effect: 'none', method: 'mixed_ols' };
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
      case 'mixed_ols':
        return data.mixed_ols;
      case 'fe_time':
        return data.fe_time;
      case 'fe_twoway':
        return data.fe_twoway;
      case 'lsdv_twoway':
        return data.lsdv_twoway;
      case 'lsdv':
        return data.lsdv;
      case 'lsdv_time':
        return data.lsdv_time;
      case 'fd':
        return data.fd;
      case 're_fgls':
        return data.re_fgls;
      case 're_mle':
        return data.re_mle;
      case 're_be':
        return data.re_be;
      case 're_fgls_time':
        return data.re_fgls_time;
      case 're_mle_time':
        return data.re_mle_time;
      case 're_be_time':
        return data.re_be_time;
      case 're_fgls_twoway':
        return data.re_fgls_twoway;
      case 're_mle_twoway':
        return data.re_mle_twoway;
      default:
        return data.mixed_ols ?? data.fe ?? data.fe_time ?? data.fe_twoway ?? data.lsdv ?? data.lsdv_time ?? data.lsdv_twoway ?? data.fd ?? data.re_fgls ?? data.re_mle ?? data.re_be ?? data.re_fgls_time ?? data.re_mle_time ?? data.re_be_time ?? data.re_fgls_twoway ?? data.re_mle_twoway;
    }
  }, [currentMethod, data]);

  const currentError = useMemo(() => {
    return data.errors?.[currentMethod];
  }, [currentMethod, data.errors]);


  const hasCategorical = useMemo(
    () => (currentData ? currentData.coefficients.some((c) => c.category != null) : false),
    [currentData]
  );

  const handleModelChange = (m: ModelType) => {
    setModelType(m);
    const newEffect: EffectType = m === 'mixed' ? 'none' : (effectType === 'none' ? 'entity' : effectType);
    setEffectType(newEffect);
    const first = METHOD_MAP[m][newEffect][0];
    if (first) setActiveMethod(first.key);
  };

  const handleEffectChange = (e: EffectType) => {
    setEffectType(e);
    const first = METHOD_MAP[modelType][e][0];
    if (first) setActiveMethod(first.key);
  };

  return (
    <ReportLayout
      title={data.title}
      badges={
        <span className="text-xs text-muted-foreground">
          {data.endog_name} · Entity ID + Time ID (cluster by entity)
        </span>
      }
    >

      {/* Model selector card: 2 rows */}
      {data.selection_tests && data.selection_tests.length > 0 && (
        <PanelSelectionTestsBlock tests={data.selection_tests} />
      )}
      <div className="mb-6 rounded-xl border border-border bg-muted/30 overflow-hidden">
        {/* Row 1: Model Type (left) | Effect Type (right) */}
        <div className="flex flex-col sm:flex-row sm:divide-x sm:divide-border">
          <div className="flex-1 p-4 flex flex-col items-start">
            <div className="text-[11px] text-muted-foreground uppercase tracking-wider mb-2.5 font-medium">
              Model Type
            </div>
            <ToggleGroup
              type="single"
              value={modelType}
              onValueChange={(value) => value && handleModelChange(value as ModelType)}
              variant="outline"
              size="sm"
              className="flex-wrap justify-start"
            >
              {MODEL_TYPE_TABS.map(({ key, label }) => (
                <ToggleGroupItem key={key} value={key} className="px-3.5 text-sm">
                  {label}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </div>
          <div className="flex-1 p-4 flex flex-col items-end">
            <div className="text-[11px] text-muted-foreground uppercase tracking-wider mb-2.5 font-medium">
              Effect Type
            </div>
            {modelType === 'mixed' ? (
              <div className="text-sm text-muted-foreground border border-border rounded-lg px-3.5 py-1.5">
                Not Applicable
              </div>
            ) : (
              <ToggleGroup
                type="single"
                value={effectType}
                onValueChange={(value) => value && handleEffectChange(value as EffectType)}
                variant="outline"
                size="sm"
                className="flex-wrap justify-end"
              >
                {EFFECT_TYPE_TABS.map(({ key, label }) => (
                  <ToggleGroupItem key={key} value={key} className="px-3.5 text-sm">
                    {label}
                  </ToggleGroupItem>
                ))}
              </ToggleGroup>
            )}
          </div>
        </div>

        {/* Row 2: Estimation Method */}
        <div className="border-t border-border p-4 bg-card/50">
          <div className="text-[11px] text-muted-foreground uppercase tracking-wider mb-2.5 font-medium">
            Estimation Method
          </div>
          <ToggleGroup
            type="single"
            value={currentMethod}
            onValueChange={(value) => value && setActiveMethod(value as TabKey)}
            variant="outline"
            size="sm"
            className="flex-wrap"
          >
            {methods.map(({ key, label }) => {
              const hasErr = data.errors?.[key];
              return (
                <ToggleGroupItem key={key} value={key} className="gap-1.5 px-3.5 text-sm">
                  {label}
                  {hasErr && (
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <span className="text-red-400">⚠</span>
                      </TooltipTrigger>
                      <TooltipContent side="top">{hasErr}</TooltipContent>
                    </Tooltip>
                  )}
                </ToggleGroupItem>
              );
            })}
          </ToggleGroup>
        </div>
      </div>

      <ReportSection title="Equation" icon="equation">
        <ReportLazyBoundary variant="formula">
          <LazyPanelFormulaBlock modelType={modelType} effectType={effectType} method={currentMethod} />
        </ReportLazyBoundary>
      </ReportSection>

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
              <div className="text-sm text-foreground mt-1">{currentError}</div>
            </div>
          </div>
        </div>
      )}

      {/* Result content */}
      {currentData && !currentError && (
        <>
          <ReportSection title="Model Summary" icon="modelSummary">
          {(currentMethod === 're_fgls' || currentMethod === 're_mle' || currentMethod === 're_fgls_time' || currentMethod === 're_mle_time' || currentMethod === 're_fgls_twoway' || currentMethod === 're_mle_twoway') &&
          currentData?.diagnostic_info?.panel_fe_info ? (
            <PanelRESummaryGrid
              info={currentData.model_basic_info}
              panelFe={currentData.diagnostic_info.panel_fe_info}
            />
          ) : (currentMethod === 're_be' || currentMethod === 're_be_time') && currentData?.diagnostic_info?.panel_fe_info ? (
            <PanelBESummaryGrid
              info={currentData.model_basic_info}
              panelFe={currentData.diagnostic_info.panel_fe_info}
              effectType={currentMethod === 're_be_time' ? 'time' : 'entity'}
            />
          ) : (currentMethod === 'fe' || currentMethod === 'fe_time') &&
            currentData?.diagnostic_info?.panel_fe_info ? (
            <PanelFESummaryGrid
              info={currentData.model_basic_info}
              panelFe={currentData.diagnostic_info.panel_fe_info}
            />
          ) : (
            <ModelSummaryGrid info={currentData.model_basic_info} />
          )}
          </ReportSection>

          {/* Coefficients */}
          <CoefficientsBlock
            coefficients={currentData.coefficients}
            hasCategorical={hasCategorical}
            useZStat={
              currentData.model_basic_info?.wald_chi2 != null ||
              currentData.model_basic_info?.lr_chi2 != null
            }
          />

          {currentData.diagnostic_info ? (
            <OmittedVariablesAlert diag={currentData.diagnostic_info} />
          ) : null}

          <HypothesisTestBlock data={currentData as OLSResultData} />

          {(currentMethod === 're_mle' || currentMethod === 're_mle_time' || currentMethod === 're_mle_twoway') &&
            (currentData.model_basic_info?.mle_iter_log_lik_const != null ||
              currentData.model_basic_info?.mle_iter_log_lik != null) && (
              <ReportSection title="MLE Iteration Log" icon="document">
                <PanelMLEIterationBlock info={currentData.model_basic_info} />
              </ReportSection>
            )}
        </>
      )}
    </ReportLayout>
  );
};
