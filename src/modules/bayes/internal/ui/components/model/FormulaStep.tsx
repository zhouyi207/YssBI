import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { BayesModelDraftDTO, LikelihoodSpecDTO } from "@/shared/types/bayes";
import type { FormulaParseError } from "@/features/application/bayes";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { formatExpression, formatRawExpressionLatex } from "@/features/domain/bayes";
import { bayesErrorMessage } from "../../bayesIssuePresentation";
import { PanelTitle, replaceAt } from "./BayesFields";
import { LatexFormulaPreview, RecognizedSymbols, latexSymbol } from "./LatexPresentation";
import type { Translation } from "./types";

export function FormulaStep({
  draft,
  onModelEquationChange,
  onErrorClear,
  error,
}: {
  draft: BayesModelDraftDTO;
  onModelEquationChange: (formulaText: string, likelihood: LikelihoodSpecDTO) => Promise<boolean>;
  onErrorClear: () => void;
  error: FormulaParseError | null;
}) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [responseExpression, setResponseExpression] = useState(currentResponseExpression(draft));
  const [distribution, setDistribution] = useState<LikelihoodDistribution>(
    likelihoodDistribution(draft.likelihood),
  );
  const [distributionArgs, setDistributionArgs] = useState<string[]>(() =>
    initialDistributionArgs(draft),
  );

  useEffect(() => {
    if (!editing) {
      setResponseExpression(currentResponseExpression(draft));
      setDistribution(likelihoodDistribution(draft.likelihood));
      setDistributionArgs(initialDistributionArgs(draft));
    }
  }, [draft, editing]);

  const applyDistribution = (nextDistribution: LikelihoodDistribution) => {
    setDistribution(nextDistribution);
    setDistributionArgs((currentArgs) =>
      resizeDistributionArgs(nextDistribution, currentArgs, draft),
    );
  };

  const commit = async () => {
    const nextResponse = responseExpression.trim() || "y";
    const nextFormulaText = composeLikelihoodLatex(nextResponse, distribution, distributionArgs);
    const saved = await onModelEquationChange(
      nextFormulaText,
      likelihoodFromFormulaParts(distribution, distributionArgs, draft.likelihood),
    );
    if (saved) setEditing(false);
  };

  const cancel = () => {
    setResponseExpression(currentResponseExpression(draft));
    setDistribution(likelihoodDistribution(draft.likelihood));
    setDistributionArgs(initialDistributionArgs(draft));
    onErrorClear();
    setEditing(false);
  };

  return (
    <Card>
      <CardHeader className="flex-row items-start justify-between gap-3">
        <PanelTitle title={t("bayes.formula.title")} />
        <Button size="sm" variant="outline" onClick={() => setEditing(true)} disabled={editing}>
          {t("bayes.actions.edit")}
        </Button>
      </CardHeader>
      <CardContent className="space-y-3">
        {error ? <FormulaErrorFeedback error={error} /> : null}
        <div className="space-y-1.5">
          {editing ? (
            <div className="space-y-3 rounded-md border border-border bg-muted/20 p-3">
              <div className="grid gap-3 md:grid-cols-[120px_180px_minmax(0,1fr)]">
                <div className="space-y-1.5">
                  <Label
                    htmlFor="bayes-response-expression"
                    className="text-xs text-muted-foreground"
                  >
                    {t("bayes.formula.responseExpression")}
                  </Label>
                  <Input
                    id="bayes-response-expression"
                    value={responseExpression}
                    autoFocus
                    className="h-8 font-mono"
                    onChange={(event) => setResponseExpression(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Escape") {
                        event.preventDefault();
                        cancel();
                      }
                    }}
                  />
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">
                    {t("bayes.formula.distribution")}
                  </Label>
                  <Select
                    value={distribution}
                    onValueChange={(value) => applyDistribution(value as LikelihoodDistribution)}
                  >
                    <SelectTrigger size="sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="normal">Normal</SelectItem>
                      <SelectItem value="bernoulli_logit">BernoulliLogit</SelectItem>
                      <SelectItem value="poisson_log">PoissonLog</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div className="grid gap-2 md:grid-cols-2">
                  {distributionArgLabels(distribution, t).map((label, index) => (
                    <div key={label} className="space-y-1.5">
                      <Label className="text-xs text-muted-foreground">{label}</Label>
                      <Input
                        value={distributionArgs[index] ?? ""}
                        className="h-8 font-mono"
                        onChange={(event) =>
                          setDistributionArgs((current) =>
                            replaceAt(current, index, event.target.value),
                          )
                        }
                        onKeyDown={(event) => {
                          if (event.key === "Enter" && event.ctrlKey) commit();
                          if (event.key === "Escape") {
                            event.preventDefault();
                            cancel();
                          }
                        }}
                      />
                    </div>
                  ))}
                </div>
              </div>
              <LatexFormulaPreview
                formulaText={composeLikelihoodLatex(
                  responseExpression || "y",
                  distribution,
                  distributionArgs,
                )}
              />
              <RecognizedSymbols symbols={draft.symbols.map((symbol) => symbol.name)} t={t} />
              <div className="flex justify-end gap-2">
                <Button size="sm" variant="outline" onClick={cancel}>
                  {t("bayes.actions.cancel")}
                </Button>
                <Button size="sm" onClick={commit}>
                  {t("bayes.actions.save")}
                </Button>
              </div>
            </div>
          ) : (
            <div className="space-y-2">
              <LatexFormulaPreview formulaText={draft.formulaText} />
              <RecognizedSymbols symbols={draft.symbols.map((symbol) => symbol.name)} t={t} />
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function FormulaErrorFeedback({ error }: { error: FormulaParseError }) {
  const { t } = useTranslation();
  return (
    <div role="alert" className="space-y-1 text-xs text-destructive">
      <p>
        <span className="font-mono">[{error.code}]</span> {bayesErrorMessage(error, t)}
      </p>
      {error.details?.column ? (
        <p>{t("bayes.issue.column", { column: error.details.column })}</p>
      ) : null}
      {typeof error.details?.row === "number" ? (
        <p>{t("bayes.issue.row", { row: error.details.row + 1 })}</p>
      ) : null}
      {error.details?.parameter ? (
        <p>{t("bayes.issue.parameter", { parameter: error.details.parameter })}</p>
      ) : null}
      {error.details?.path ? <p>{t("bayes.issue.path", { path: error.details.path })}</p> : null}
      {error.incidentId ? (
        <p>
          {t("common.incidentId")}: <span className="font-mono">{error.incidentId}</span>
        </p>
      ) : null}
    </div>
  );
}

type LikelihoodDistribution = LikelihoodSpecDTO["type"];

export function currentResponseExpression(draft: BayesModelDraftDTO): string {
  return formatRawExpressionLatex(draft.rawResponse) || "y";
}

function likelihoodDistribution(likelihood: LikelihoodSpecDTO): LikelihoodDistribution {
  return likelihood.type;
}

function initialDistributionArgs(draft: BayesModelDraftDTO): string[] {
  const predictor =
    extractPredictorLatex(draft.formulaText) ||
    formatExpression(draft.boundPredictor) ||
    "a \\cdot x + b";
  switch (draft.likelihood.type) {
    case "normal":
      return [predictor, latexSymbol(draft.likelihood.sigma.parameter)];
    case "bernoulli_logit":
    case "poisson_log":
      return [predictor];
  }
}

function resizeDistributionArgs(
  distribution: LikelihoodDistribution,
  currentArgs: string[],
  draft: BayesModelDraftDTO,
): string[] {
  const argumentCount = distributionArgumentCount(distribution);
  const fallbackArgs = initialDistributionArgs({
    ...draft,
    likelihood: likelihoodFromDistribution(distribution),
  });
  return Array.from(
    { length: argumentCount },
    (_, index) => currentArgs[index] ?? fallbackArgs[index] ?? "",
  );
}

function distributionArgumentCount(distribution: LikelihoodDistribution): number {
  return distribution === "normal" ? 2 : 1;
}

function distributionArgLabels(distribution: LikelihoodDistribution, t: Translation): string[] {
  switch (distribution) {
    case "normal":
      return [t("bayes.formula.meanPredictor"), t("bayes.formula.standardDeviationSigma")];
    case "bernoulli_logit":
      return [t("bayes.formula.logit")];
    case "poisson_log":
      return [t("bayes.formula.logRate")];
  }
}

export function composeLikelihoodLatex(
  responseExpression: string,
  distribution: LikelihoodDistribution,
  args: string[],
): string {
  const response = responseExpression.trim() || "y";
  const safeArgs = Array.from(
    { length: distributionArgumentCount(distribution) },
    (_, index) => args[index]?.trim() || "\\cdots",
  );
  return `${response} \\sim \\operatorname{${distributionLatexName(distribution)}}\\left(${safeArgs.join(", ")}\\right)`;
}

function likelihoodFromFormulaParts(
  distribution: LikelihoodDistribution,
  args: string[],
  current: LikelihoodSpecDTO,
): LikelihoodSpecDTO {
  switch (distribution) {
    case "normal":
      return {
        type: "normal",
        mean: { source: "predictor" },
        sigma: {
          parameter:
            latexToPlainSymbol(args[1]) ||
            (current.type === "normal" ? current.sigma.parameter : "sigma"),
        },
      };
    case "bernoulli_logit":
      return { type: "bernoulli_logit", logit: { source: "predictor" } };
    case "poisson_log":
      return { type: "poisson_log", logRate: { source: "predictor" } };
  }
}

function likelihoodFromDistribution(distribution: LikelihoodDistribution): LikelihoodSpecDTO {
  switch (distribution) {
    case "normal":
      return { type: "normal", mean: { source: "predictor" }, sigma: { parameter: "sigma" } };
    case "bernoulli_logit":
      return { type: "bernoulli_logit", logit: { source: "predictor" } };
    case "poisson_log":
      return { type: "poisson_log", logRate: { source: "predictor" } };
  }
}

function distributionLatexName(distribution: LikelihoodDistribution): string {
  switch (distribution) {
    case "normal":
      return "Normal";
    case "bernoulli_logit":
      return "BernoulliLogit";
    case "poisson_log":
      return "PoissonLog";
  }
}

function extractPredictorLatex(formulaText: string): string | null {
  const trimmed = formulaText.trim();
  const equalsIndex = trimmed.indexOf("=");
  if (equalsIndex >= 0) return trimmed.slice(equalsIndex + 1).trim() || null;
  const normalMatch = trimmed.match(
    /\\operatorname\{(?:Normal|BernoulliLogit|PoissonLog)\}\\left\((.*)\\right\)$/,
  );
  if (normalMatch?.[1]) return normalMatch[1].split(",")[0]?.trim() || null;
  return null;
}

function latexToPlainSymbol(value: string | undefined): string | null {
  const trimmed = value?.trim();
  if (!trimmed) return null;
  if (trimmed === "\\sigma") return "sigma";
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(trimmed) ? trimmed : null;
}
