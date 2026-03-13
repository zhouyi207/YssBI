import React from 'react';
import katex from 'katex';
import 'katex/dist/katex.min.css';

type PanelMethod = 'fe' | 'fe_time' | 'fe_twoway' | 'lsdv' | 'lsdv_time' | 'lsdv_twoway' | 'fd' | 're_fgls' | 're_mle' | 're_be' | 're_fgls_time' | 're_mle_time' | 're_be_time' | 're_fgls_twoway' | 're_mle_twoway';
type ModelType = 'fe' | 're';
type EffectType = 'entity' | 'time' | 'twoway';

function renderKatex(latex: string, displayMode = true): string | null {
  try {
    return katex.renderToString(latex, { displayMode, throwOnError: false });
  } catch {
    return null;
  }
}

function renderInlineKatex(latex: string): string | null {
  return renderKatex(latex, false);
}

// Part 1: Core formula (Model Type + Effect Type)
const CORE_FORMULAS: Record<ModelType, Record<EffectType, string>> = {
  fe: {
    entity: `y_{it} = X_{it}'\\beta + \\alpha_i + u_{it}`,
    time: `y_{it} = X_{it}'\\beta + \\lambda_t + u_{it}`,
    twoway: `y_{it} = X_{it}'\\beta + \\alpha_i + \\lambda_t + u_{it}`,
  },
  re: {
    entity: `y_{it} = X_{it} \\beta + \\varepsilon_{it} \\\\
    \\varepsilon_{it} = \\alpha_i + u_{it}`,
    time: `y_{it} = X_{it} \\beta + \\varepsilon_{it} \\\\
    \\varepsilon_{it} = \\lambda_t + u_{it}`,
    twoway: `y_{it} = X_{it} \\beta + \\varepsilon_{it} \\\\
    \\varepsilon_{it} = \\alpha_i + \\lambda_t + u_{it}`,
  },
};

// Part 2: Estimation method formula
const METHOD_FORMULAS: Record<PanelMethod, string> = {
  fe: `(y_{it} - \\bar{y}_i) = (X_{it} - \\bar{X}_i)'\\beta + (u_{it} - \\bar{u}_i)`,
  fe_time: `(y_{it} - \\bar{y}_t) = (X_{it} - \\bar{X}_t)'\\beta + (u_{it} - \\bar{u}_t)`,
  fe_twoway: `\\tilde{y}_{it} = \\tilde{X}_{it}'\\beta + \\tilde{u}_{it},\\quad \\tilde{z}_{it} = z_{it} - \\bar{z}_i - \\bar{z}_t + \\bar{z}`,
  lsdv: `y_{it} = \\alpha + X_{it}'\\beta + \\sum_{i=2}^{n} \\gamma_i D_i + u_{it}`,
  lsdv_time: `y_{it} = \\alpha + X_{it}'\\beta + \\sum_{t=2}^{T} \\gamma_t D_t + u_{it}`,
  lsdv_twoway: `y_{it} = \\alpha + X_{it}'\\beta + \\sum_{i=2}^{n} \\gamma_i D_i + \\sum_{t=2}^{T} \\lambda_t D_t + u_{it}`,
  fd: `\\Delta y_{it} = \\Delta X_{it}'\\beta + \\Delta u_{it},\\quad \\Delta z_{it} = z_{it} - z_{i,t-1}`,
  re_fgls: `y_{it}^* = y_{it} - \\theta_i \\bar{y}_i,\\quad X_{it}^* = X_{it} - \\theta_i \\bar{X}_i,\\quad \\theta_i = 1 - \\sqrt{\\frac{\\sigma_e^2}{T_i\\sigma_u^2 + \\sigma_e^2}}`,
  re_mle: `y_{it}^* = y_{it} - \\theta_i \\bar{y}_i,\\quad X_{it}^* = X_{it} - \\theta_i \\bar{X}_i,\\quad \\theta_i = 1 - \\sqrt{\\frac{\\sigma_e^2}{T_i\\sigma_u^2 + \\sigma_e^2}}`,
  re_be: `\\bar{y}_i = \\bar{X}_i'\\beta + \\bar{\\varepsilon}_i,\\quad \\hat{\\beta} = (\\bar{X}'\\bar{X})^{-1}\\bar{X}'\\bar{y}`,
  re_fgls_time: `y_{it}^* = y_{it} - \\theta_t \\bar{y}_t,\\quad X_{it}^* = X_{it} - \\theta_t \\bar{X}_t,\\quad \\theta_t = 1 - \\sqrt{\\frac{\\sigma_e^2}{N_t\\sigma_u^2 + \\sigma_e^2}}`,
  re_mle_time: `y_{it}^* = y_{it} - \\theta_t \\bar{y}_t,\\quad X_{it}^* = X_{it} - \\theta_t \\bar{X}_t,\\quad \\theta_t = 1 - \\sqrt{\\frac{\\sigma_e^2}{N_t\\sigma_u^2 + \\sigma_e^2}}`,
  re_be_time: `\\bar{y}_t = \\bar{X}_t'\\beta + \\bar{\\varepsilon}_t,\\quad \\hat{\\beta} = (\\bar{X}'\\bar{X})^{-1}\\bar{X}'\\bar{y}`,
  re_fgls_twoway: `y_{it}^* = y_{it} - \\theta_i \\bar{y}_i - \\theta_t \\bar{y}_t + \\theta_{it}\\bar{y},\\quad \\varepsilon_{it} = \\alpha_i + \\lambda_t + u_{it}`,
  re_mle_twoway: `y_{it}^* = y_{it} - \\theta_i \\bar{y}_i - \\theta_t \\bar{y}_t + \\theta_{it}\\bar{y},\\quad \\varepsilon_{it} = \\alpha_i + \\lambda_t + u_{it}`,
};

const MAPPINGS_BASE = [
  { symbol: 'y_{it}', variable: 'dependent variable' },
  { symbol: '\\beta', variable: 'coefficient vector' },
  { symbol: 'X_{it}', variable: 'independent variables' },
  { symbol: 'i', variable: 'entity' },
  { symbol: 't', variable: 'time' },
  { symbol: 'u_{it}', variable: 'idiosyncratic error' },
];

const MAPPINGS_FE = [
  ...MAPPINGS_BASE,
  { symbol: '\\alpha_i', variable: 'individual fixed effect' },
];

const MAPPINGS_FE_TIME = [
  ...MAPPINGS_BASE,
  { symbol: '\\lambda_t', variable: 'time fixed effect' },
];

const MAPPINGS_FE_TWOWAY = [
  ...MAPPINGS_BASE,
  { symbol: '\\alpha_i', variable: 'individual fixed effect' },
  { symbol: '\\lambda_t', variable: 'time fixed effect' },
];

const MAPPINGS_LSDV = [
  ...MAPPINGS_BASE,
  { symbol: '\\alpha', variable: 'intercept' },
  { symbol: '\\gamma_i', variable: 'coefficient for entity i dummy' },
  { symbol: 'D_i', variable: 'dummy variable (1 if entity i, 0 else)' },
];

const MAPPINGS_RE = [
  ...MAPPINGS_BASE,
  { symbol: '\\varepsilon_{it}', variable: 'composite error' },
  { symbol: '\\alpha_i', variable: 'individual random effect' },
  { symbol: '\\theta_i', variable: 'quasi-demeaning weight' },
  { symbol: '\\sigma_u^2,\\sigma_e^2', variable: 'variance components' },
  { symbol: '\\ell', variable: 'log-likelihood' },
  { symbol: 'SSR_w,SSR_b', variable: 'within/between sum of squared residuals' },
  { symbol: '\\bar{T}', variable: 'harmonic mean of T_i' },
];

const MAPPINGS_OTHER = [
  ...MAPPINGS_BASE,
  { symbol: 'u_i', variable: 'individual effect' },
];

const MAPPINGS_LSDV_TIME = [
  ...MAPPINGS_BASE,
  { symbol: '\\alpha', variable: 'intercept' },
  { symbol: '\\gamma_t', variable: 'coefficient for time t dummy' },
  { symbol: 'D_t', variable: 'dummy variable (1 if time t, 0 else)' },
];

const MAPPINGS_LSDV_TWOWAY = [
  ...MAPPINGS_BASE,
  { symbol: '\\alpha', variable: 'intercept' },
  { symbol: '\\gamma_i', variable: 'coefficient for entity i dummy' },
  { symbol: 'D_i', variable: 'entity dummy (1 if entity i, 0 else)' },
  { symbol: '\\lambda_t', variable: 'coefficient for time t dummy' },
  { symbol: 'D_t', variable: 'time dummy (1 if time t, 0 else)' },
];

function getMappings(method: PanelMethod) {
  if (method === 'fe') return MAPPINGS_FE;
  if (method === 'fe_time') return MAPPINGS_FE_TIME;
  if (method === 'fe_twoway') return MAPPINGS_FE_TWOWAY;
  if (method === 're_fgls' || method === 're_mle' || method === 're_be' || method === 're_fgls_time' || method === 're_mle_time' || method === 're_be_time' || method === 're_fgls_twoway' || method === 're_mle_twoway') return MAPPINGS_RE;
  if (method === 'lsdv') return MAPPINGS_LSDV;
  if (method === 'lsdv_time') return MAPPINGS_LSDV_TIME;
  if (method === 'lsdv_twoway') return MAPPINGS_LSDV_TWOWAY;
  return MAPPINGS_OTHER;
}

interface PanelFormulaBlockProps {
  modelType: ModelType;
  effectType: EffectType;
  method: PanelMethod;
}

const PanelFormulaBlock: React.FC<PanelFormulaBlockProps> = ({ modelType, effectType, method }) => {
  const coreLatex = CORE_FORMULAS[modelType][effectType];
  const methodLatex = METHOD_FORMULAS[method] ?? METHOD_FORMULAS.fe;

  const coreMultiline = coreLatex.includes('\\\\');
  const methodMultiline = methodLatex.includes('\\\\');
  const coreHtml = renderKatex(
    coreMultiline ? `\\begin{gathered} ${coreLatex} \\end{gathered}` : coreLatex
  );
  const methodHtml = renderKatex(
    methodMultiline ? `\\begin{gathered} ${methodLatex} \\end{gathered}` : methodLatex
  );
  const mappings = getMappings(method);

  if (!coreHtml || !methodHtml) return null;

  return (
    <div className="rounded-lg border border-gray-800/50 bg-[#13151a] overflow-hidden w-full">
      <div className="w-full overflow-x-auto">
        <div className="min-w-full w-max px-6 py-4 space-y-5">
          {/* Part 1: Core formula */}
          <div className="w-full flex flex-col items-center">
            <div className="text-[11px] text-gray-500 uppercase tracking-wider mb-2 font-medium">
              Core Model
            </div>
            <div
              className="w-full flex justify-center [&_.katex]:text-gray-200 [&_.katex]:text-base"
              dangerouslySetInnerHTML={{ __html: coreHtml }}
            />
          </div>

          {/* Part 2: Estimation method formula */}
          <div className="w-full flex flex-col items-center">
            <div className="text-[11px] text-gray-500 uppercase tracking-wider mb-2 font-medium">
              Estimation
            </div>
            <div
              className="w-full flex justify-center [&_.katex]:text-gray-200 [&_.katex]:text-base"
              dangerouslySetInnerHTML={{ __html: methodHtml }}
            />
          </div>
        </div>
      </div>
      <div className="border-t border-gray-800/40 px-4 pb-4 pt-3">
        <div className="text-[11px] text-gray-500 uppercase tracking-wider mb-2 px-1">Variable Mapping</div>
        <table className="w-full text-xs">
          <thead>
            <tr className="text-gray-500">
              <th className="text-left px-3 py-1.5 font-medium w-20">Symbol</th>
              <th className="text-left px-3 py-1.5 font-medium">Variable</th>
            </tr>
          </thead>
          <tbody>
            {mappings.map((m, idx) => {
              const symHtml = renderInlineKatex(m.symbol);
              return (
                <tr key={`${m.symbol}-${idx}`} className={`border-t border-gray-800/20 ${idx % 2 === 0 ? 'bg-[#15171d]/50' : ''}`}>
                  <td className="px-3 py-1.5">
                    {symHtml ? (
                      <span className="[&_.katex]:text-[var(--accent-color)]" dangerouslySetInnerHTML={{ __html: symHtml }} />
                    ) : (
                      <span className="font-mono text-[var(--accent-color)]">{m.symbol}</span>
                    )}
                  </td>
                  <td className="px-3 py-1.5 font-mono text-gray-300">{m.variable}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
};

export default PanelFormulaBlock;
