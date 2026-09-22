import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import {
  LINEAR_SUMMARY_CONTENTS,
  type LinearSummaryAddition,
  type LinearSummaryContent,
  type LinearSummaryOptions,
} from "@/shared/types/domain/resultReport";
import type { ErrorReference } from "@/features/application/errorReference";

export function AddReportContents({
  options,
  paramNames,
  busy,
  error,
  onAdd,
}: {
  options: LinearSummaryOptions;
  paramNames: string[];
  busy: boolean;
  error: ErrorReference | null;
  onAdd: (additions: LinearSummaryAddition) => Promise<void>;
}) {
  const { t } = useTranslation();
  const id = useId();
  const [selected, setSelected] = useState<Partial<Record<LinearSummaryContent, true>>>({});
  const [acfLag, setAcfLag] = useState(String(options.acf_max_lag));
  const [serialLag, setSerialLag] = useState(String(options.serial_lags));
  const [nomiss0, setNomiss0] = useState(options.bg_nomiss0);
  const [hypothesis, setHypothesis] = useState(options.hypothesis);
  const available = LINEAR_SUMMARY_CONTENTS.filter(({ key }) => !options[key]);
  const validLag = (value: string) =>
    Number.isInteger(Number(value)) && Number(value) >= 1 && Number(value) <= 40;
  const valid =
    Object.keys(selected).length > 0 &&
    (!selected.acf_pacf || validLag(acfLag)) &&
    (!selected.serial_tests || validLag(serialLag)) &&
    (!selected.hypothesis_test ||
      (hypothesis.trim().length > 0 && new TextEncoder().encode(hypothesis).length <= 4096));
  const errorKey =
    error?.code === "report_summary_changed"
      ? "changed"
      : error?.code === "report_summary_unavailable"
        ? "unavailable"
        : "failed";
  return (
    <details
      className="mb-5 rounded-lg border border-border p-3"
      open={busy || error !== null || undefined}
    >
      <summary className="cursor-pointer text-sm font-medium">{t("reportSummary.add")}</summary>
      <form
        className="mt-3 space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          if (!valid || busy) return;
          void onAdd({
            ...selected,
            ...(selected.acf_pacf ? { acf_max_lag: Number(acfLag) } : {}),
            ...(selected.serial_tests
              ? { serial_lags: Number(serialLag), bg_nomiss0: nomiss0 }
              : {}),
            ...(selected.hypothesis_test ? { hypothesis: hypothesis.trim() } : {}),
          });
        }}
      >
        <p className="text-xs text-muted-foreground">{t("reportSummary.help")}</p>
        {available.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t("reportSummary.allIncluded")}</p>
        ) : (
          <fieldset disabled={busy} className="space-y-4">
            <div className="grid gap-3 sm:grid-cols-2">
              {available.map(({ key, section }) => (
                <label
                  key={key}
                  className="flex items-center gap-2 text-sm"
                  htmlFor={`${id}-${key}`}
                >
                  <Checkbox
                    id={`${id}-${key}`}
                    checked={selected[key] === true}
                    disabled={busy}
                    onCheckedChange={(checked) =>
                      setSelected((previous) => {
                        const next = { ...previous };
                        if (checked === true) next[key] = true;
                        else delete next[key];
                        return next;
                      })
                    }
                  />
                  {t(`reportLayout.sections.${section}`)}
                </label>
              ))}
            </div>
            {selected.acf_pacf && (
              <label className="flex items-center gap-3 text-sm" htmlFor={`${id}-acf-lag`}>
                {t("reportSummary.acfLag")}
                <Input
                  id={`${id}-acf-lag`}
                  type="number"
                  min={1}
                  max={40}
                  step={1}
                  required
                  value={acfLag}
                  onChange={(event) => setAcfLag(event.target.value)}
                  className="w-24"
                />
              </label>
            )}
            {selected.serial_tests && (
              <div className="space-y-3">
                <label className="flex items-center gap-3 text-sm" htmlFor={`${id}-serial-lag`}>
                  {t("reportSummary.serialLag")}
                  <Input
                    id={`${id}-serial-lag`}
                    type="number"
                    min={1}
                    max={40}
                    step={1}
                    required
                    value={serialLag}
                    onChange={(event) => setSerialLag(event.target.value)}
                    className="w-24"
                  />
                </label>
                <label className="flex items-center gap-2 text-sm" htmlFor={`${id}-nomiss0`}>
                  <Checkbox
                    id={`${id}-nomiss0`}
                    checked={nomiss0}
                    disabled={busy}
                    onCheckedChange={(checked) => setNomiss0(checked === true)}
                  />
                  {t("reportSummary.nomiss0")}
                </label>
              </div>
            )}
            {selected.hypothesis_test && (
              <div className="space-y-2">
                <label htmlFor={`${id}-hypothesis`} className="text-sm">
                  {t("reportSummary.hypothesis")}
                </label>
                <Input
                  id={`${id}-hypothesis`}
                  required
                  maxLength={4096}
                  value={hypothesis}
                  onChange={(event) => setHypothesis(event.target.value)}
                />
                <p className="text-xs text-muted-foreground">
                  {t("reportSummary.paramNames", { names: paramNames.join(", ") })}
                </p>
              </div>
            )}
            <Button type="submit" size="sm" disabled={!valid || busy}>
              {t(busy ? "reportSummary.computing" : "reportSummary.compute")}
            </Button>
          </fieldset>
        )}
        {busy && (
          <p role="status" className="text-sm text-muted-foreground">
            {t("reportSummary.pending")}
          </p>
        )}
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {t(`reportSummary.${errorKey}`)}{" "}
            <span className="text-xs">
              [{error.code}]{error.incidentId ? ` · ${error.incidentId}` : ""}
            </span>
          </p>
        )}
      </form>
    </details>
  );
}
