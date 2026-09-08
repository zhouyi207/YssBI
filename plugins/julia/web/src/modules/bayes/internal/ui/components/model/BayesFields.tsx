import { useTranslation } from "react-i18next";
import type { ValidationIssueDTO } from "@/shared/types/bayes";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { bayesValidationIssueMessage } from "../../bayesIssuePresentation";

export function PanelTitle({
  title,
  description,
  issues = [],
}: {
  title: string;
  description?: string;
  issues?: ValidationIssueDTO[];
}) {
  const { t } = useTranslation();
  return (
    <div className="space-y-1">
      <h2 className="text-sm font-semibold text-foreground">{title}</h2>
      {description ? <p className="text-xs text-muted-foreground">{description}</p> : null}
      {issues.map((issue) => (
        <p
          key={`${issue.code}-${issue.path}`}
          className={
            issue.severity === "error"
              ? "text-xs text-destructive"
              : "text-xs text-muted-foreground"
          }
        >
          <span className="font-mono">[{issue.code}]</span> {bayesValidationIssueMessage(issue, t)}
        </p>
      ))}
    </div>
  );
}

export function EditableNumberField({
  label,
  value,
  min,
  max,
  step = 1,
  onChange,
}: {
  label: string;
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange: (value: string) => void;
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <Input
        type="number"
        value={value}
        min={min}
        max={max}
        step={step}
        className="h-9"
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

export function ReadOnlyField({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <div
        className={`rounded-md border border-border bg-muted/30 px-3 py-2 text-sm ${mono ? "font-mono" : ""}`}
      >
        {value}
      </div>
    </div>
  );
}

export function replaceAt(values: string[], index: number, value: string): string[] {
  const next = [...values];
  next[index] = value;
  return next;
}

export function formatNumber(value: number): string {
  return Number.isFinite(value) ? value.toFixed(3) : "—";
}
