import type { VARStableRow } from "@/shared/types/report/var";

function statusFromModulus(modulus: number): "stable" | "unstable" {
  return modulus >= 1 ? "unstable" : "stable";
}

export function toVarStabilityPoints(rows: readonly VARStableRow[]) {
  return rows.map((row) => ({
    re: row.re,
    im: row.im,
    modulus: row.modulus,
    status: statusFromModulus(row.modulus),
  }));
}
