import { TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import type { UiTableData } from "@/shared/types/domain/uiData";
import {
  InfoStatsTable,
  infoStatsHeadClass,
  infoStatsHeadCompactClass,
  infoStatsRowEvenClass,
  infoStatsRowOddClass,
  infoStatsCellClass,
  infoStatsCellRightClass,
} from "./TableFrame";
import { formatValue, valueClass } from "./formatValue";

export function DataTable({ columns, rows }: Omit<UiTableData, "kind">) {
  return (
    <InfoStatsTable className="mb-2">
      <TableHeader>
        <TableRow className="border-0 hover:bg-transparent">
          {columns.map((column) => (
            <TableHead
              key={column.id}
              className={column.format === "text" ? infoStatsHeadClass : infoStatsHeadCompactClass}
            >
              {column.label}
            </TableHead>
          ))}
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.map((row, index) => (
          <TableRow
            key={index}
            className={index % 2 === 0 ? infoStatsRowEvenClass : infoStatsRowOddClass}
          >
            {columns.map((column, cell) => (
              <TableCell
                key={column.id}
                className={`${column.format === "text" ? infoStatsCellClass : infoStatsCellRightClass} ${valueClass(row[cell], column.format)}`}
              >
                {formatValue(row[cell], column.format)}
              </TableCell>
            ))}
          </TableRow>
        ))}
      </TableBody>
    </InfoStatsTable>
  );
}
