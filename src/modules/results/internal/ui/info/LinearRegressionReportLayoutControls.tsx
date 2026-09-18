import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { VscArrowDown, VscArrowUp } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  defaultLinearRegressionReportSpec,
  LINEAR_REGRESSION_REPORT_SPEC_TEXT_LIMIT,
  parseLinearRegressionReportSpecJson,
  type LinearRegressionReportSpec,
  type LinearRegressionReportSpecIssue,
} from "@/shared/types/domain/linearRegressionReportSpec";

export function LinearRegressionReportLayoutControls({
  spec,
  onChange,
}: {
  spec: LinearRegressionReportSpec;
  onChange: (spec: LinearRegressionReportSpec) => void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const [text, setText] = useState("");
  const [issue, setIssue] = useState<LinearRegressionReportSpecIssue | null>(null);
  const install = (next: LinearRegressionReportSpec) => {
    onChange(next);
    setIssue(null);
  };
  const move = (index: number, offset: number) => {
    const sections = [...spec.sections];
    [sections[index], sections[index + offset]] = [sections[index + offset], sections[index]];
    install({ ...spec, sections });
  };

  return (
    <details className="mb-5 rounded-lg border border-border p-3">
      <summary className="cursor-pointer text-sm font-medium">{t("reportLayout.title")}</summary>
      <div className="mt-3 space-y-3">
        <p className="text-xs text-muted-foreground">{t("reportLayout.sessionOnly")}</p>
        <ul className="space-y-1">
          {spec.sections.map((section, index) => {
            const label = t(`reportLayout.sections.${section.kind}`);
            return (
              <li key={section.id} className="flex items-center gap-2 rounded px-1 py-0.5">
                <Checkbox
                  id={`${id}-${section.id}`}
                  checked={section.visible}
                  onCheckedChange={(checked) =>
                    install({
                      ...spec,
                      sections: spec.sections.map((item) =>
                        item.id === section.id ? { ...item, visible: checked === true } : item,
                      ),
                    })
                  }
                />
                <label htmlFor={`${id}-${section.id}`} className="flex-1 cursor-pointer text-sm">
                  {label}
                </label>
                <Button
                  type="button"
                  variant="ghost"
                  size="xs"
                  aria-label={t("reportLayout.moveUp", { section: label })}
                  disabled={index === 0}
                  onClick={() => move(index, -1)}
                >
                  <VscArrowUp aria-hidden />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="xs"
                  aria-label={t("reportLayout.moveDown", { section: label })}
                  disabled={index === spec.sections.length - 1}
                  onClick={() => move(index, 1)}
                >
                  <VscArrowDown aria-hidden />
                </Button>
              </li>
            );
          })}
        </ul>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => install(defaultLinearRegressionReportSpec(spec.source))}
        >
          {t("common.restoreDefaults")}
        </Button>
        <details className="border-t border-border pt-3">
          <summary className="cursor-pointer text-xs">{t("reportLayout.json")}</summary>
          <div className="mt-3 space-y-2">
            <label htmlFor={`${id}-json`} className="block text-xs text-muted-foreground">
              {t("reportLayout.jsonHelp")}
            </label>
            <textarea
              id={`${id}-json`}
              value={text}
              onChange={(event) => setText(event.target.value)}
              maxLength={LINEAR_REGRESSION_REPORT_SPEC_TEXT_LIMIT + 1}
              spellCheck={false}
              rows={10}
              className="w-full rounded-md border border-input bg-background p-2 font-mono text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
              aria-invalid={issue !== null}
              aria-describedby={issue ? `${id}-error` : undefined}
            />
            <div className="flex flex-wrap gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => {
                  setText(JSON.stringify(spec, null, 2));
                  setIssue(null);
                }}
              >
                {t("reportLayout.export")}
              </Button>
              <Button
                type="button"
                size="sm"
                disabled={!text.trim()}
                onClick={() => {
                  const parsed = parseLinearRegressionReportSpecJson(text, spec.source);
                  if (parsed.ok) install(parsed.value);
                  else setIssue(parsed.issue);
                }}
              >
                {t("reportLayout.apply")}
              </Button>
            </div>
            {issue ? (
              <p id={`${id}-error`} role="alert" className="text-xs text-destructive">
                {t(`reportLayout.errors.${issue.code}`, { path: issue.path })}
              </p>
            ) : null}
          </div>
        </details>
      </div>
    </details>
  );
}
