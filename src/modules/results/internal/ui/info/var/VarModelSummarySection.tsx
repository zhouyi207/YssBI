import { ReportSection } from "../shared/ReportLayout";
import { formatNum } from "../shared/RegressionShared";

export function VarModelSummarySection({
  completeSampleRows,
  varMaxLag,
  numObservation,
  logLikelihood,
  aic,
  fpe,
  hqic,
  sbic,
  detSigmaMl,
}: {
  completeSampleRows: number | null | undefined;
  varMaxLag: number | null | undefined;
  numObservation: number;
  logLikelihood: number;
  aic: number;
  fpe: number;
  hqic: number;
  sbic: number;
  detSigmaMl: number;
}) {
  return (
    <ReportSection title="Model Summary" icon="modelSummary">
      <div className="mb-6 grid grid-cols-2 gap-px overflow-hidden rounded-lg border border-border bg-border">
        {completeSampleRows != null && varMaxLag != null ? (
          <div className="col-span-2 flex justify-between border-b border-border bg-card px-4 py-2.5">
            <span className="shrink-0 text-xs text-muted-foreground">Observations</span>
            <span className="text-right font-mono text-xs text-foreground">
              T = {completeSampleRows}, p = {varMaxLag}, n = {numObservation}{" "}
              <span className="font-sans text-muted-foreground">
                （无缺失外生时 n = T − p；首期外生缺失不减少 n）
              </span>
            </span>
          </div>
        ) : null}
        <SummaryRow label="Log likelihood" value={formatNum(logLikelihood)} />
        <SummaryRow label="AIC" value={formatNum(aic)} />
        <SummaryRow label="FPE" value={formatNum(fpe)} />
        <SummaryRow label="HQIC" value={formatNum(hqic)} />
        <SummaryRow label="SBIC" value={formatNum(sbic)} />
        <SummaryRow label="Det(Sigma_ml)" value={formatNum(detSigmaMl)} />
      </div>
    </ReportSection>
  );
}

function SummaryRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between bg-card px-4 py-2.5">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="font-mono text-xs font-medium text-foreground">{value}</span>
    </div>
  );
}
