import React, { useMemo, useState } from "react";
import katex from "katex";
import "katex/dist/katex.min.css";
import { ScrollArea } from "@/components/ui/scroll-area";
import { InfoSegmentedToggle } from "@/components/ui-presentation/Controls";
import { FormulaMappingTable } from "@/components/ui-presentation/FormulaMappingTable";
import { formatNum } from "@/shared/stats/formatStat";
import type { Coefficient } from "@/shared/types/report";

function escapeLatex(s: string): string {
  return s.replace(/[_{}\\^~&%$#]/g, (ch) => `\\${ch}`);
}

type EquationMode = "expanded" | "symbolic";

interface VariableMapping {
  symbol: string;
  variable: string;
  coef: number;
}

function buildExpandedLatex(endogName: string, coefficients: Coefficient[]): string {
  const lhs = `\\text{${escapeLatex(endogName)}}`;
  const terms: string[] = [];

  for (const c of coefficients) {
    const coefStr = formatNum(c.coef);
    if (c.variable === "const") {
      terms.push(coefStr);
      continue;
    }
    const absCoef = formatNum(Math.abs(c.coef));
    const sign = c.coef >= 0 ? "+" : "-";
    const varLabel = `\\text{${escapeLatex(c.variable)}}`;

    if (terms.length === 0) {
      terms.push(`${coefStr} \\cdot ${varLabel}`);
    } else {
      terms.push(`${sign} ${absCoef} \\cdot ${varLabel}`);
    }
  }

  return `${lhs} = ${terms.join(" ")} + \\varepsilon`;
}

function buildSymbolicData(endogName: string, coefficients: Coefficient[]) {
  const mappings: VariableMapping[] = [];
  const terms: string[] = [];
  let xi = 1;

  mappings.push({ symbol: "y", variable: endogName, coef: NaN });

  for (const c of coefficients) {
    if (c.variable === "const") {
      mappings.push({ symbol: "\\beta_0", variable: "const", coef: c.coef });
      terms.push("\\beta_0");
      continue;
    }
    const sym = `x_{${xi}}`;
    const beta = `\\beta_{${xi}}`;
    mappings.push({ symbol: sym, variable: c.variable, coef: c.coef });
    terms.push(`${beta} ${sym}`);
    xi++;
  }

  return { latex: `y = ${terms.join(" + ")} + \\varepsilon`, mappings };
}

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

interface EquationProps {
  endogName: string;
  coefficients: Coefficient[];
}

const Equation: React.FC<EquationProps> = ({ endogName, coefficients }) => {
  const [mode, setMode] = useState<EquationMode>("symbolic");

  const expandedHtml = useMemo(
    () => renderKatex(buildExpandedLatex(endogName, coefficients)),
    [endogName, coefficients],
  );

  const { symbolicHtml, mappings } = useMemo(() => {
    const { latex, mappings } = buildSymbolicData(endogName, coefficients);
    return { symbolicHtml: renderKatex(latex), mappings };
  }, [endogName, coefficients]);

  return (
    <div className="rounded-lg border border-border bg-card overflow-hidden">
      {/* Toggle */}
      <div className="flex items-center justify-end px-4 pt-3 pb-0">
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
      <ScrollArea orientation="horizontal">
        <div
          className="px-6 py-4 w-max min-w-full [&_.katex]:text-foreground"
          dangerouslySetInnerHTML={{
            __html: (mode === "expanded" ? expandedHtml : symbolicHtml) || "",
          }}
        />
      </ScrollArea>

      {/* Mapping table (symbolic mode only) */}
      {mode === "symbolic" && (
        <div className="border-t border-border px-4 pb-4 pt-3">
          <div className="mb-2 px-1 text-[11px] uppercase tracking-wider text-muted-foreground">
            Variable Mapping
          </div>
          <FormulaMappingTable
            mappings={mappings}
            renderSymbol={(symbol) => {
              const symHtml = renderInlineKatex(symbol);
              return symHtml ? (
                <span
                  className="[&_.katex]:text-[var(--accent-color)]"
                  dangerouslySetInnerHTML={{ __html: symHtml }}
                />
              ) : (
                <span className="font-mono text-[var(--accent-color)]">{symbol}</span>
              );
            }}
          />
        </div>
      )}
    </div>
  );
};

export default Equation;
