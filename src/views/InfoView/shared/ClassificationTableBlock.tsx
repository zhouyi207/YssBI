import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { formatNum, formatPercent } from './RegressionShared';
import type { ClassificationTable } from './types';

/** Stata estat classification — classification table and statistics */
export function ClassificationTableBlock({ data }: { data: ClassificationTable }) {
  const totalD = data.tp + data.fn_;
  const totalND = data.tn + data.fp;
  const totalPos = data.tp + data.fp;
  const totalNeg = data.tn + data.fn_;
  const total = data.tp + data.fp + data.tn + data.fn_;

  return (
    <div className="rounded-xl border border-border bg-card overflow-hidden shadow-sm">
      <div className="px-4 py-3 border-b border-border">
        <h3 className="text-sm font-medium text-foreground">Classification Table (estat classification)</h3>
        <p className="text-xs text-muted-foreground mt-0.5">
          Classified + if predicted Pr(D) ≥ {data.cutoff}
        </p>
      </div>

      {/* 2×2 Table */}
      <div className="p-4">
        <Table className="w-full border-collapse text-sm">
          <TableHeader>
            <TableRow className="border-b border-border hover:bg-transparent">
              <TableHead className="h-auto px-3 py-2 text-left font-medium text-muted-foreground"></TableHead>
              <TableHead className="h-auto px-3 py-2 text-center font-medium text-muted-foreground">True D</TableHead>
              <TableHead className="h-auto px-3 py-2 text-center font-medium text-muted-foreground">True ~D</TableHead>
              <TableHead className="h-auto px-3 py-2 text-center font-medium text-muted-foreground">Total</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow className="border-b border-border">
              <TableCell className="px-3 py-2 font-medium text-muted-foreground">Classified +</TableCell>
              <TableCell className="px-3 py-2 text-center font-mono text-emerald-400">{data.tp}</TableCell>
              <TableCell className="px-3 py-2 text-center font-mono text-amber-400">{data.fp}</TableCell>
              <TableCell className="px-3 py-2 text-center font-mono text-foreground">{totalPos}</TableCell>
            </TableRow>
            <TableRow>
              <TableCell className="px-3 py-2 font-medium text-muted-foreground">Classified −</TableCell>
              <TableCell className="px-3 py-2 text-center font-mono text-amber-400">{data.fn_}</TableCell>
              <TableCell className="px-3 py-2 text-center font-mono text-emerald-400">{data.tn}</TableCell>
              <TableCell className="px-3 py-2 text-center font-mono text-foreground">{totalNeg}</TableCell>
            </TableRow>
            <TableRow className="border-t border-border">
              <TableCell className="px-3 py-2 font-medium text-muted-foreground">Total</TableCell>
              <TableCell className="px-3 py-2 text-center font-mono text-foreground">{totalD}</TableCell>
              <TableCell className="px-3 py-2 text-center font-mono text-foreground">{totalND}</TableCell>
              <TableCell className="px-3 py-2 text-center font-mono text-foreground">{total}</TableCell>
            </TableRow>
          </TableBody>
        </Table>

        {/* Statistics */}
        <div className="mt-4 space-y-1.5 text-xs">
          {[
            { label: 'Sensitivity', formula: 'Pr(+|D)', value: data.sensitivity },
            { label: 'Specificity', formula: 'Pr(−|~D)', value: data.specificity },
            { label: 'Positive predictive value', formula: 'Pr(D|+)', value: data.ppv },
            { label: 'Negative predictive value', formula: 'Pr(~D|−)', value: data.npv },
            { label: 'False + rate for true ~D', formula: 'Pr(+|~D)', value: data.false_pos_rate },
            { label: 'False − rate for true D', formula: 'Pr(−|D)', value: data.false_neg_rate },
          ].map(({ label, formula, value }) => (
            <div key={label} className="flex justify-between items-center py-1.5 border-b border-border">
              <span className="text-muted-foreground">{label}</span>
              <span className="text-muted-foreground font-mono text-[11px] mr-4">{formula}</span>
              <span className="text-foreground font-mono font-medium">{formatPercent(value)}</span>
            </div>
          ))}
          <div className="flex justify-between py-2 mt-2 bg-muted/40 rounded px-3">
            <span className="text-muted-foreground font-medium">Correctly classified</span>
            <span className="text-[var(--accent-color)] font-mono font-semibold">{formatNum(data.pct_correct, 2)}%</span>
          </div>
        </div>
      </div>
    </div>
  );
}
