import type {
  BayesDatasetSelectionDTO,
  BayesSymbolRoleDTO,
  ParameterConstraintDTO,
  PriorSpecDTO,
} from "@/shared/types/bayes";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { replaceAt } from "./BayesFields";
import { LatexInline, latexSymbol } from "./LatexPresentation";
import {
  isPriorCompatibleWithConstraint,
  numericColumns,
  priorArgumentCount,
} from "./symbolConfigValues";
import type { BayesDatasetOption, Translation } from "./types";

export function SymbolConfigDialog({
  open,
  datasets,
  symbol,
  selectedDatasetId,
  role,
  detailValue,
  constraint,
  priorDistribution,
  priorArgs,
  onDatasetChange,
  onRoleChange,
  onDetailValueChange,
  onConstraintChange,
  onPriorDistributionChange,
  onPriorArgsChange,
  onClose,
  onSave,
  t,
}: {
  open: boolean;
  datasets: readonly BayesDatasetOption[];
  symbol: string | null;
  selectedDatasetId: string;
  role: BayesSymbolRoleDTO;
  detailValue: string;
  constraint: ParameterConstraintDTO;
  priorDistribution: PriorSpecDTO["distribution"];
  priorArgs: string[];
  onDatasetChange: (sourceId: string) => void;
  onRoleChange: (role: BayesSymbolRoleDTO) => void;
  onDetailValueChange: (value: string) => void;
  onConstraintChange: (constraint: ParameterConstraintDTO) => void;
  onPriorDistributionChange: (distribution: PriorSpecDTO["distribution"]) => void;
  onPriorArgsChange: (args: string[]) => void;
  onClose: () => void;
  onSave: () => void;
  t: Translation;
}) {
  const selectedDataset =
    datasets.find((dataset) => dataset.sourceId === selectedDatasetId) ?? null;
  const selectedColumns = numericColumns(Array.from(selectedDataset?.columns ?? []));
  const priorLabels = priorArgLabels(priorDistribution, t);

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
      <DialogContent
        explicitClose
        className="grid max-h-[85vh] w-[min(calc(100vw-2rem),44rem)] max-w-none grid-rows-[auto_minmax(0,1fr)_auto] rounded-lg"
      >
        <DialogHeader className="flex flex-row items-center justify-between gap-3 border-b border-border bg-muted/30">
          <DialogTitle>{t("bayes.symbols.configurationTitle")}</DialogTitle>
          <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            aria-label={t("bayes.actions.close")}
            className="ml-auto"
            onClick={onClose}
          >
            <span aria-hidden="true" className="text-lg leading-none">
              ×
            </span>
          </Button>
        </DialogHeader>
        <div className="min-h-0 space-y-4 overflow-y-auto px-6 py-5">
          <section className="grid gap-4 rounded-md border border-border bg-muted/10 p-4 md:grid-cols-[minmax(8rem,1fr)_minmax(0,2fr)]">
            <div className="flex min-w-0 items-center gap-3">
              <Label className="w-14 shrink-0 text-xs font-medium text-muted-foreground">
                {t("bayes.symbols.symbol")}
              </Label>
              <div className="min-w-0 flex-1 truncate text-sm font-medium text-foreground">
                <LatexInline formulaText={latexSymbol(symbol ?? "")} />
              </div>
            </div>
            <div className="flex min-w-0 items-center gap-3">
              <Label className="w-10 shrink-0 text-xs font-medium text-muted-foreground">
                {t("bayes.symbols.role")}
              </Label>
              <div className="min-w-0 flex-1">
                <SymbolRoleSelect value={role} onChange={onRoleChange} t={t} />
              </div>
            </div>
          </section>

          {role === "parameter" ? (
            <div className="space-y-4">
              <section className="space-y-3 rounded-md border border-border p-4">
                <div className="flex items-center justify-between gap-3">
                  <h3 className="text-xs font-semibold uppercase tracking-wide text-foreground">
                    {t("bayes.constraints.parameterConstraint")}
                  </h3>
                  <span className="text-xs text-muted-foreground">
                    <LatexInline
                      formulaText={`${latexSymbol(symbol ?? "parameter")} \\in ${constraintSetLatex(constraint)}`}
                    />
                  </span>
                </div>
                <div className="flex w-full items-center gap-3">
                  <Label className="w-18 shrink-0 text-xs text-muted-foreground">
                    {t("bayes.constraints.constraint")}
                  </Label>
                  <div className="w-48 max-w-full">
                    <ConstraintSelect
                      value={constraint.type}
                      onChange={(type) => onConstraintChange(defaultConstraint(type, constraint))}
                      t={t}
                    />
                  </div>
                </div>
                <BoundsEditor constraint={constraint} onChange={onConstraintChange} t={t} />
              </section>
              <section className="space-y-3 rounded-md border border-border p-4">
                <div className="flex items-center justify-between gap-3">
                  <h3 className="text-xs font-semibold uppercase tracking-wide text-foreground">
                    {t("bayes.prior.title")}
                  </h3>
                  <span className="text-xs text-muted-foreground">
                    <LatexInline
                      formulaText={priorSummaryLatex(symbol, priorDistribution, priorArgs)}
                    />
                  </span>
                </div>
                <div className="flex items-center gap-3">
                  <Label className="w-24 shrink-0 text-xs text-muted-foreground">
                    {t("bayes.prior.distribution")}
                  </Label>
                  <div className="w-64 max-w-full">
                    <Select
                      value={priorDistribution}
                      onValueChange={(value) =>
                        onPriorDistributionChange(value as PriorSpecDTO["distribution"])
                      }
                    >
                      <SelectTrigger>
                        <SelectValue placeholder={t("bayes.prior.selectDistribution")} />
                      </SelectTrigger>
                      <SelectContent>
                        {priorDistributionsForConstraint(constraint).map((distribution) => (
                          <SelectItem key={distribution} value={distribution}>
                            {priorDistributionLabel(distribution)}
                            {isPriorCompatibleWithConstraint(distribution, constraint)
                              ? ` · ${t("bayes.prior.recommended")}`
                              : ""}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>
                <div className={`grid gap-3 ${priorParameterGridClass(priorLabels.length)}`}>
                  {priorLabels.map((label, index) => (
                    <div key={label} className="space-y-1.5">
                      <Label className="text-xs text-muted-foreground">{label}</Label>
                      <Input
                        aria-label={label}
                        value={priorArgs[index] ?? ""}
                        className="font-mono"
                        onChange={(event) =>
                          onPriorArgsChange(replaceAt(priorArgs, index, event.target.value))
                        }
                      />
                    </div>
                  ))}
                </div>
              </section>
            </div>
          ) : (
            <section className="space-y-4 rounded-md border border-border p-4">
              <h3 className="text-xs font-semibold uppercase tracking-wide text-foreground">
                {t("bayes.dataBinding.title")}
              </h3>
              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">
                    {t("bayes.dataBinding.dataSource")}
                  </Label>
                  <Select
                    value={selectedDatasetId}
                    onValueChange={(sourceId) => {
                      onDatasetChange(sourceId);
                      const dataset = datasets.find((item) => item.sourceId === sourceId);
                      const nextColumn = dataset
                        ? preferredSymbolColumn(dataset, symbol ?? "")
                        : null;
                      onDetailValueChange(nextColumn ?? "");
                    }}
                  >
                    <SelectTrigger>
                      <SelectValue placeholder={t("bayes.dataBinding.selectDataSource")} />
                    </SelectTrigger>
                    <SelectContent>
                      {datasets.map((dataset) => (
                        <SelectItem key={dataset.sourceId} value={dataset.sourceId}>
                          {dataset.displayName}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  {datasets.length === 0 ? (
                    <p className="text-xs text-muted-foreground">
                      {t("bayes.dataBinding.noDataSources")}
                    </p>
                  ) : null}
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">
                    {t("bayes.dataBinding.dataColumn")}
                  </Label>
                  <SymbolDetailEditor
                    columns={selectedColumns}
                    value={detailValue}
                    onValueChange={onDetailValueChange}
                    t={t}
                  />
                  {selectedDataset && selectedColumns.length === 0 ? (
                    <p className="text-xs text-muted-foreground">
                      {t("bayes.dataBinding.noColumns")}
                    </p>
                  ) : null}
                </div>
              </div>
            </section>
          )}
        </div>
        <DialogFooter className="shrink-0">
          <Button variant="outline" onClick={onClose}>
            {t("bayes.actions.cancel")}
          </Button>
          <Button onClick={onSave}>{t("bayes.actions.save")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function SymbolRoleSelect({
  value,
  onChange,
  t,
}: {
  value: BayesSymbolRoleDTO;
  onChange: (role: BayesSymbolRoleDTO) => void;
  t: Translation;
}) {
  return (
    <Select value={value} onValueChange={(nextValue) => onChange(nextValue as BayesSymbolRoleDTO)}>
      <SelectTrigger className="w-40 max-w-full">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="dependent">{t("bayes.roles.dependent")}</SelectItem>
        <SelectItem value="independent">{t("bayes.roles.independent")}</SelectItem>
        <SelectItem value="parameter">{t("bayes.roles.parameter")}</SelectItem>
      </SelectContent>
    </Select>
  );
}

function preferredSymbolColumn(
  dataset: { readonly columns: readonly BayesDatasetOption["columns"][number][] },
  symbolName: string,
): string | null {
  const columns = numericColumns(Array.from(dataset.columns));
  return columns.find((column) => column.name === symbolName)?.name ?? columns[0]?.name ?? null;
}

function SymbolDetailEditor({
  columns,
  value,
  onValueChange,
  t,
}: {
  columns: BayesDatasetSelectionDTO["columns"];
  value: string;
  onValueChange: (value: string) => void;
  t: Translation;
}) {
  return (
    <Select value={value} onValueChange={onValueChange} disabled={columns.length === 0}>
      <SelectTrigger>
        <SelectValue placeholder={t("bayes.symbols.selectDataColumn")} />
      </SelectTrigger>
      <SelectContent>
        {columns.map((column) => (
          <SelectItem key={column.name} value={column.name}>
            {column.name} · {column.dtype}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

const PRIOR_DISTRIBUTIONS: PriorSpecDTO["distribution"][] = [
  "normal",
  "log_normal",
  "uniform",
  "beta",
  "gamma",
  "exponential",
  "student_t",
  "cauchy",
  "half_normal",
];

function priorDistributionsForConstraint(
  constraint: ParameterConstraintDTO,
): PriorSpecDTO["distribution"][] {
  const recommended = PRIOR_DISTRIBUTIONS.filter((distribution) =>
    isPriorCompatibleWithConstraint(distribution, constraint),
  );
  const others = PRIOR_DISTRIBUTIONS.filter((distribution) => !recommended.includes(distribution));
  return [...recommended, ...others];
}

function ConstraintSelect({
  value,
  onChange,
  t,
}: {
  value: ParameterConstraintDTO["type"];
  onChange: (type: ParameterConstraintDTO["type"]) => void;
  t: Translation;
}) {
  return (
    <Select
      value={value}
      onValueChange={(nextValue) => onChange(nextValue as ParameterConstraintDTO["type"])}
    >
      <SelectTrigger>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="real">{t("bayes.constraints.real")}</SelectItem>
        <SelectItem value="positive">{t("bayes.constraints.positive")}</SelectItem>
        <SelectItem value="unit">{t("bayes.constraints.unit")}</SelectItem>
        <SelectItem value="bounded">{t("bayes.constraints.bounded")}</SelectItem>
      </SelectContent>
    </Select>
  );
}

function BoundsEditor({
  constraint,
  onChange,
  t,
}: {
  constraint: ParameterConstraintDTO;
  onChange: (constraint: ParameterConstraintDTO) => void;
  t: Translation;
}) {
  if (constraint.type !== "bounded") return null;

  return (
    <div className="grid gap-3 md:grid-cols-4">
      <div className="space-y-1.5">
        <Label className="text-xs text-muted-foreground">{t("bayes.constraints.lowerBound")}</Label>
        <Input
          type="number"
          value={constraint.lower}
          className="font-mono"
          onChange={(event) => onChange({ ...constraint, lower: Number(event.target.value) })}
        />
      </div>
      <div className="space-y-1.5">
        <Label className="text-xs text-muted-foreground">{t("bayes.constraints.upperBound")}</Label>
        <Input
          type="number"
          value={constraint.upper}
          className="font-mono"
          onChange={(event) => onChange({ ...constraint, upper: Number(event.target.value) })}
        />
      </div>
      <BooleanSelect
        label={t("bayes.constraints.includeLower")}
        value={constraint.includeLower}
        onChange={(includeLower) => onChange({ ...constraint, includeLower })}
        t={t}
      />
      <BooleanSelect
        label={t("bayes.constraints.includeUpper")}
        value={constraint.includeUpper}
        onChange={(includeUpper) => onChange({ ...constraint, includeUpper })}
        t={t}
      />
    </div>
  );
}

function BooleanSelect({
  label,
  value,
  onChange,
  t,
}: {
  label: string;
  value: boolean;
  onChange: (value: boolean) => void;
  t: Translation;
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <Select value={String(value)} onValueChange={(nextValue) => onChange(nextValue === "true")}>
        <SelectTrigger>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="true">{t("bayes.constraints.true")}</SelectItem>
          <SelectItem value="false">{t("bayes.constraints.false")}</SelectItem>
        </SelectContent>
      </Select>
    </div>
  );
}

function defaultConstraint(
  type: ParameterConstraintDTO["type"],
  previous: ParameterConstraintDTO,
): ParameterConstraintDTO {
  switch (type) {
    case "real":
      return { type: "real" };
    case "positive":
      return { type: "positive" };
    case "unit":
      return { type: "unit" };
    case "bounded":
      return previous.type === "bounded"
        ? previous
        : { type: "bounded", lower: 0, upper: 1, includeLower: false, includeUpper: false };
  }
}

function constraintSetLatex(constraint: ParameterConstraintDTO): string {
  switch (constraint.type) {
    case "real":
      return "(-\\infty, \\infty)";
    case "positive":
      return "(0, \\infty)";
    case "unit":
      return "(0, 1)";
    case "bounded": {
      const left = constraint.includeLower ? "[" : "(";
      const right = constraint.includeUpper ? "]" : ")";
      return `${left}${constraint.lower}, ${constraint.upper}${right}`;
    }
  }
}

function priorSummaryLatex(
  symbol: string | null,
  distribution: PriorSpecDTO["distribution"],
  args: readonly string[],
): string {
  const distributionName = priorDistributionLabel(distribution);
  const values = args.slice(0, priorArgumentCount(distribution)).map((value) => value || "\\cdots");
  return `${latexSymbol(symbol ?? "parameter")} \\sim \\operatorname{${distributionName}}\\left(${values.join(", ")}\\right)`;
}

function priorDistributionLabel(distribution: PriorSpecDTO["distribution"]): string {
  return distribution
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");
}

function priorParameterGridClass(parameterCount: number): string {
  if (parameterCount === 1) return "md:grid-cols-1";
  if (parameterCount === 2) return "md:grid-cols-2";
  return "md:grid-cols-3";
}

function priorArgLabels(distribution: PriorSpecDTO["distribution"], t: Translation): string[] {
  switch (distribution) {
    case "normal":
      return [t("bayes.prior.args.mean"), t("bayes.prior.args.standardDeviation")];
    case "log_normal":
      return [t("bayes.prior.args.logMean"), t("bayes.prior.args.logStandardDeviation")];
    case "uniform":
      return [t("bayes.prior.args.lowerBound"), t("bayes.prior.args.upperBound")];
    case "beta":
      return [t("bayes.prior.args.alpha"), t("bayes.prior.args.beta")];
    case "gamma":
      return [t("bayes.prior.args.shape"), t("bayes.prior.args.scale")];
    case "exponential":
      return [t("bayes.prior.args.scale")];
    case "student_t":
      return [
        t("bayes.prior.args.degreesOfFreedom"),
        t("bayes.prior.args.location"),
        t("bayes.prior.args.scale"),
      ];
    case "cauchy":
      return [t("bayes.prior.args.location"), t("bayes.prior.args.scale")];
    case "half_normal":
      return [t("bayes.prior.args.scale")];
  }
}
