import { TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { formatNum } from "./RegressionShared";
import type { ModelBasicInfo } from "@/shared/types/report";
import {
  InfoStatsTable,
  infoStatsCellClass,
  infoStatsCellRightClass,
  infoStatsHeadClass,
  infoStatsHeadCompactClass,
  infoStatsRowEvenClass,
  infoStatsRowOddClass,
} from "./InfoStatsTable";

export function AnovaTable({ info }: { info: ModelBasicInfo }) {
  return (
    <InfoStatsTable className="mb-2">
      <TableHeader>
        <TableRow className="border-0 hover:bg-transparent">
          <TableHead className={infoStatsHeadClass}>Source</TableHead>
          <TableHead className={infoStatsHeadCompactClass}>SS</TableHead>
          <TableHead className={infoStatsHeadCompactClass}>df</TableHead>
          <TableHead className={infoStatsHeadCompactClass}>MS</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow className={infoStatsRowEvenClass}>
          <TableCell className={`${infoStatsCellClass} font-mono text-foreground`}>Model</TableCell>
          <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
            {formatNum(info.ss_model)}
          </TableCell>
          <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
            {info.df_model}
          </TableCell>
          <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
            {formatNum(info.ms_model)}
          </TableCell>
        </TableRow>
        <TableRow className={infoStatsRowOddClass}>
          <TableCell className={`${infoStatsCellClass} font-mono text-foreground`}>
            Residual
          </TableCell>
          <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
            {formatNum(info.ss_residual)}
          </TableCell>
          <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
            {info.df_residual}
          </TableCell>
          <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
            {formatNum(info.ms_residual)}
          </TableCell>
        </TableRow>
        <TableRow className={infoStatsRowEvenClass}>
          <TableCell className={`${infoStatsCellClass} font-mono font-semibold text-foreground`}>
            Total
          </TableCell>
          <TableCell className={`${infoStatsCellRightClass} font-semibold text-foreground`}>
            {formatNum(info.ss_total)}
          </TableCell>
          <TableCell className={`${infoStatsCellRightClass} font-semibold text-foreground`}>
            {info.df_total}
          </TableCell>
          <TableCell className={`${infoStatsCellRightClass} font-semibold text-foreground`}>
            {formatNum(info.ms_total)}
          </TableCell>
        </TableRow>
      </TableBody>
    </InfoStatsTable>
  );
}
