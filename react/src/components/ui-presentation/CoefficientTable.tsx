import { TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { formatNum } from "@/shared/stats/formatStat";
import type { Coefficient } from "@/shared/types/report";
import {
  InfoStatsTable,
  infoStatsCellClass,
  infoStatsCellRightClass,
  infoStatsHeadClass,
  infoStatsHeadCompactClass,
  infoStatsRowEvenClass,
  infoStatsRowOddClass,
} from "@/components/ui-presentation/TableFrame";

export function CoefficientTable({ coefficients }: { coefficients: Coefficient[] }) {
  return (
    <>
      <InfoStatsTable>
        <TableHeader>
          <TableRow className="border-0 hover:bg-transparent">
            <TableHead className={infoStatsHeadClass}>Variable</TableHead>
            <TableHead className={infoStatsHeadCompactClass}>Coef</TableHead>
            <TableHead className={infoStatsHeadCompactClass}>Std Err</TableHead>
            <TableHead className={infoStatsHeadCompactClass}>t</TableHead>
            <TableHead className={infoStatsHeadCompactClass}>P&gt;|t|</TableHead>
            <TableHead className={infoStatsHeadCompactClass}>[0.025</TableHead>
            <TableHead className={infoStatsHeadCompactClass}>0.975]</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {coefficients.map((coeff, idx) => {
            return (
              <TableRow
                key={`${coeff.variable}-${idx}`}
                className={idx % 2 === 0 ? infoStatsRowEvenClass : infoStatsRowOddClass}
              >
                <TableCell className={infoStatsCellClass}>
                  <div className="flex items-center gap-2">
                    <div
                      className={`h-1.5 w-1.5 rounded-full ${coeff.is_significant ? "bg-emerald-400" : "bg-muted-foreground/40"}`}
                    />
                    <span
                      className={`font-mono font-medium ${coeff.is_significant ? "text-foreground" : "text-muted-foreground"}`}
                    >
                      {coeff.variable}
                    </span>
                  </div>
                </TableCell>
                <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
                  {formatNum(coeff.coef)}
                </TableCell>
                <TableCell className={`${infoStatsCellRightClass} text-muted-foreground`}>
                  {coeff.std_err != null ? formatNum(coeff.std_err) : "."}
                </TableCell>
                <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
                  {coeff.t_value != null ? formatNum(coeff.t_value, 3) : "."}
                </TableCell>
                <TableCell className={infoStatsCellRightClass}>
                  {coeff.p_value != null ? (
                    <>
                      <span
                        className={
                          coeff.is_significant ? "text-emerald-400" : "text-muted-foreground"
                        }
                      >
                        {formatNum(coeff.p_value, 3)}
                      </span>
                      <SignificanceStars pValue={coeff.p_value} />
                    </>
                  ) : (
                    <span className="text-muted-foreground">.</span>
                  )}
                </TableCell>
                <TableCell className={`${infoStatsCellRightClass} text-muted-foreground`}>
                  {coeff["confidence_interval_0.025"] != null
                    ? formatNum(coeff["confidence_interval_0.025"])
                    : "."}
                </TableCell>
                <TableCell className={`${infoStatsCellRightClass} text-muted-foreground`}>
                  {coeff["confidence_interval_0.975"] != null
                    ? formatNum(coeff["confidence_interval_0.975"])
                    : "."}
                </TableCell>
              </TableRow>
            );
          })}
        </TableBody>
      </InfoStatsTable>

      <div className="mt-2 flex items-center gap-4 px-1 text-[10px] text-muted-foreground">
        <span>
          Significance: <span className="text-yellow-400">***</span> p&lt;0.001,{" "}
          <span className="text-yellow-400">**</span> p&lt;0.01,{" "}
          <span className="text-yellow-400">*</span> p&lt;0.05,{" "}
          <span className="text-muted-foreground">.</span> p&lt;0.1
        </span>
      </div>
    </>
  );
}

export function SignificanceStars({ pValue }: { pValue: number }) {
  if (pValue < 0.001) return <span className="text-yellow-400 font-bold ml-1">***</span>;
  if (pValue < 0.01) return <span className="text-yellow-400 font-bold ml-1">**</span>;
  if (pValue < 0.05) return <span className="text-yellow-400 font-bold ml-1">*</span>;
  if (pValue < 0.1) return <span className="text-muted-foreground ml-1">.</span>;
  return null;
}
