import React, { useMemo, useState } from "react";
import katex from "katex";
import "katex/dist/katex.min.css";
import { ScrollArea } from "@/components/ui/scroll-area";
import { InfoSegmentedToggle } from "./shared/InfoViewControls";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatNum } from "./shared/utils";
import type { VARCoefDisplay } from "@/shared/types/report";

function escapeLatex(s: string): string {
  return s.replace(/[_{}\\^~&%$#]/g, (ch) => `\\${ch}`);
}

type EquationMode = "expanded" | "symbolic";

function renderKatex(latex: string, displayMode = true): string | null {
  try {
    return katex.renderToString(latex, { displayMode, throwOnError: false });
  } catch {
    return null;
  }
}

/** 从系数变量名解析滞后阶数 */
function parseLagsFromCoeffs(coefficients: VARCoefDisplay[]): number[] {
  const lags = new Set<number>();
  for (const c of coefficients) {
    const m = c.variable.match(/^L(\d+)\./);
    if (m) lags.add(parseInt(m[1], 10));
  }
  return Array.from(lags).sort((a, b) => a - b);
}

/** 按方程分组系数 */
function groupByEquation(coefficients: VARCoefDisplay[]): Map<string, VARCoefDisplay[]> {
  const map = new Map<string, VARCoefDisplay[]>();
  for (const c of coefficients) {
    const list = map.get(c.eq_name) ?? [];
    list.push(c);
    map.set(c.eq_name, list);
  }
  return map;
}

/** 将 L1.var 转为 LaTeX 下标形式 var_{t-1} */
function varToLatex(varName: string): string {
  const m = varName.match(/^L(\d+)\.(.+)$/);
  if (m) {
    const lag = m[1];
    const name = escapeLatex(m[2]);
    return `\\text{${name}}_{t-${lag}}`;
  }
  if (varName === "const") return "";
  return `\\text{${escapeLatex(varName)}}`;
}

function buildSymbolicLatex(lags: number[]): string {
  const sumTerms =
    lags.length > 0
      ? lags.map((l) => `A_{${l}} \\mathbf{y}_{t-${l}}`).join(" + ")
      : "A_1 \\mathbf{y}_{t-1} + \\cdots + A_p \\mathbf{y}_{t-p}";
  return `\\begin{gathered}
\\mathbf{y}_t = \\mathbf{v} + ${sumTerms} + \\mathbf{u}_t \\\\
\\mathbf{y}_t \\in \\mathbb{R}^K,\\quad A_l \\in \\mathbb{R}^{K \\times K},\\quad \\mathbf{u}_t \\sim \\text{WN}(0,\\Sigma)
\\end{gathered}`;
}

function buildExpandedLatex(varNames: string[], eqCoeffs: Map<string, VARCoefDisplay[]>): string {
  const lines: string[] = [];
  for (const eqName of varNames) {
    const coeffs = [...(eqCoeffs.get(eqName) ?? [])].sort(
      (a, b) => (a.variable === "const" ? 0 : 1) - (b.variable === "const" ? 0 : 1),
    );
    const terms: string[] = [];
    for (const c of coeffs) {
      if (c.variable === "const") {
        terms.push(formatNum(c.coef));
        continue;
      }
      const absCoef = formatNum(Math.abs(c.coef));
      const sign = c.coef >= 0 ? "+" : "-";
      const varLabel = varToLatex(c.variable);
      if (varLabel) {
        if (terms.length === 0) {
          terms.push(`${formatNum(c.coef)} \\cdot ${varLabel}`);
        } else {
          terms.push(`${sign} ${absCoef} \\cdot ${varLabel}`);
        }
      }
    }
    const lhs = `\\text{${escapeLatex(eqName)}}_t`;
    const rhs = `${terms.join(" \\; ")} + u_t`;
    lines.push(`${lhs} &= ${rhs}`);
  }
  return `\\begin{aligned}\n${lines.join(" \\\\\n")}\n\\end{aligned}`;
}

interface VARFormulaBlockProps {
  varNames: string[];
  coefficients: VARCoefDisplay[];
}

const VARFormulaBlock: React.FC<VARFormulaBlockProps> = ({ varNames, coefficients }) => {
  const [mode, setMode] = useState<EquationMode>("symbolic");

  const { symbolicHtml, expandedHtml } = useMemo(() => {
    const lags = parseLagsFromCoeffs(coefficients);
    const eqCoeffs = groupByEquation(coefficients);
    const symbolicHtml = renderKatex(buildSymbolicLatex(lags));
    const expandedHtml = renderKatex(buildExpandedLatex(varNames, eqCoeffs));
    return { symbolicHtml, expandedHtml };
  }, [varNames, coefficients]);

  return (
    <div className="rounded-lg border border-border bg-card overflow-hidden">
      {/* Toggle */}
      <div className="flex items-center justify-end px-4 pt-3 pb-1">
        <InfoSegmentedToggle
          value={mode}
          onValueChange={setMode}
          options={[
            { value: "symbolic", label: "Symbolic" },
            { value: "expanded", label: "Expanded" },
          ]}
        />
      </div>

      {/* Formula */}
      <ScrollArea orientation="both">
        <div
          className="px-6 py-5 min-w-full [&_.katex]:text-foreground [&_.katex]:text-[1.05em] [&_.katex-display]:py-3 [&_.katex-display]:leading-relaxed"
          dangerouslySetInnerHTML={{
            __html: (mode === "expanded" ? expandedHtml : symbolicHtml) || "",
          }}
        />
      </ScrollArea>

      {/* Symbolic mode: variable mapping */}
      {mode === "symbolic" && (
        <div className="border-t border-border px-4 pb-4 pt-3">
          <div className="mb-2 px-1 text-[11px] uppercase tracking-wider text-muted-foreground">
            Variable Mapping
          </div>
          <Table className="w-full text-xs">
            <TableHeader>
              <TableRow className="border-0 hover:bg-transparent">
                <TableHead className="h-auto w-24 px-3 py-1.5 text-left font-medium text-muted-foreground">
                  Symbol
                </TableHead>
                <TableHead className="h-auto px-3 py-1.5 text-left font-medium text-muted-foreground">
                  Meaning
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {[
                ["y_t", "K×1 vector of endogenous variables"],
                ["A_l", "K×K coefficient matrix at lag l"],
                ["v", "Constant vector"],
                ["u_t", "Innovation vector, white noise"],
              ].map(([symbol, meaning], idx) => (
                <TableRow
                  key={symbol}
                  className={`border-t border-border ${idx % 2 === 0 ? "bg-muted/50" : ""}`}
                >
                  <TableCell className="px-3 py-1.5 font-mono text-[var(--accent-color)]">
                    {symbol}
                  </TableCell>
                  <TableCell className="px-3 py-1.5 text-muted-foreground">{meaning}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  );
};

export default VARFormulaBlock;
