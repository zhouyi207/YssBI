import React, { useMemo } from 'react';
import { ReportLayout, ReportLazyBoundary, ReportSection, CoefficientsBlock, LazyVARFormulaBlock } from './shared';
import type { VARSummaryResultData } from '@/shared/types/report';
import {
  VarEquationSummarySection,
  VarGrangerSection,
  VarImpulseMatrixSection,
  VarLmarSection,
  VarModelSummarySection,
  VarReportSubtitle,
  VarStabilitySection,
  VarWaldLagExclusionSection,
} from './var';
import { sortVarStableRows, varCoeffsToOLSFormat } from './var/varReportUtils';

export const VARComponent: React.FC<{ data: VARSummaryResultData }> = ({ data }) => {
  const {
    var_names,
    num_observation,
    complete_sample_rows,
    var_max_lag,
    log_likelihood,
    aic,
    fpe,
    hqic,
    sbic,
    equations,
    coefficients,
    oirf,
    fevd,
    varwle,
    varlmar,
    varstable,
    vargranger,
  } = data;

  const coeffsForTable = useMemo(() => varCoeffsToOLSFormat(coefficients), [coefficients]);
  const varstableSorted = useMemo(
    () => (varstable ? sortVarStableRows(varstable) : []),
    [varstable],
  );

  return (
    <ReportLayout
      title={data.title}
      badges={
        <VarReportSubtitle
          var_names={var_names}
          complete_sample_rows={complete_sample_rows}
          var_max_lag={var_max_lag}
          num_observation={num_observation}
        />
      }
    >
      <ReportSection title="Equation" icon="equation">
        <ReportLazyBoundary variant="formula">
          <LazyVARFormulaBlock varNames={var_names} coefficients={coefficients} />
        </ReportLazyBoundary>
      </ReportSection>

      <VarModelSummarySection
        completeSampleRows={complete_sample_rows}
        varMaxLag={var_max_lag}
        numObservation={num_observation}
        logLikelihood={log_likelihood}
        aic={aic}
        fpe={fpe}
        hqic={hqic}
        sbic={sbic}
        detSigmaMl={data.det_sigma_ml}
      />

      <VarEquationSummarySection equations={equations} />

      <CoefficientsBlock
        coefficients={coeffsForTable}
        hasCategorical={true}
        useZStat={true}
        categoryLabel="Equation"
      />

      {varwle && varwle.length > 0 ? <VarWaldLagExclusionSection rows={varwle} /> : null}

      <VarStabilitySection rows={varstableSorted} />

      {vargranger && vargranger.length > 0 ? <VarGrangerSection rows={vargranger} /> : null}

      {varlmar && varlmar.length > 0 ? <VarLmarSection rows={varlmar} /> : null}

      {oirf && oirf.length > 0 && var_names.length > 0 ? (
        <VarImpulseMatrixSection
          title="Orthogonalized IRF"
          icon="irf"
          stepHeader="Step"
          varNames={var_names}
          steps={oirf}
        />
      ) : null}

      {fevd && fevd.length > 0 && var_names.length > 0 ? (
        <VarImpulseMatrixSection
          title="Forecast-error variance decomposition"
          icon="fevd"
          stepHeader="step"
          varNames={var_names}
          steps={fevd}
        />
      ) : null}
    </ReportLayout>
  );
};
