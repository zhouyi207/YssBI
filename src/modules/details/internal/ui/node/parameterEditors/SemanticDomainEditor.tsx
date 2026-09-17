import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { detailInlineInputClass } from "../../shared/detailStyles";

interface DomainDraft {
  values: { value: string; label: string }[];
  positiveValue: string | null;
}

function readDomain(value: unknown): DomainDraft {
  if (!value || typeof value !== "object") return { values: [], positiveValue: null };
  const domain = value as Record<string, unknown>;
  return {
    values: Array.isArray(domain.values)
      ? domain.values
          .filter(
            (entry): entry is DomainDraft["values"][number] =>
              !!entry &&
              typeof entry === "object" &&
              typeof entry.value === "string" &&
              typeof entry.label === "string",
          )
          .map((entry) => ({ ...entry }))
      : [],
    positiveValue: typeof domain.positiveValue === "string" ? domain.positiveValue : null,
  };
}

export function SemanticDomainEditor({
  value,
  pending,
  onCommit,
}: {
  value: unknown;
  pending: boolean;
  onCommit(value: unknown, callbacks?: { onRejected?(): void }): void;
}) {
  const { t } = useTranslation();
  const latestProjection = useRef(value);
  latestProjection.current = value;
  const [projection, setProjection] = useState(value);
  const [draft, setDraft] = useState(() => readDomain(value));
  const [page, setPage] = useState(0);
  if (projection !== value) {
    setProjection(value);
    setDraft(readDomain(value));
    setPage(0);
  }
  const unique = new Set(draft.values.map((entry) => entry.value)).size === draft.values.length;
  const lastPage = Math.max(0, Math.ceil(draft.values.length / 50) - 1);
  const shownPage = Math.min(page, lastPage);
  const start = shownPage * 50;
  const update = (index: number, field: "value" | "label", text: string) => {
    setDraft((current) => ({
      ...current,
      values: current.values.map((entry, row) =>
        row === index ? { ...entry, [field]: text } : entry,
      ),
      positiveValue:
        field === "value" && current.positiveValue === current.values[index].value
          ? text
          : current.positiveValue,
    }));
  };
  const move = (index: number, direction: number) => {
    setPage(Math.floor((index + direction) / 50));
    setDraft((current) => {
      const values = [...current.values];
      [values[index], values[index + direction]] = [values[index + direction], values[index]];
      return { ...current, values };
    });
  };
  const remove = (index: number) => {
    setDraft((current) => ({
      values: current.values.filter((_, row) => row !== index),
      positiveValue:
        current.positiveValue === current.values[index].value ? null : current.positiveValue,
    }));
  };
  return (
    <div className="min-w-0 space-y-2">
      <p className="text-xs text-muted-foreground">{t("conversion.domainHelp")}</p>
      <div className="max-h-64 space-y-2 overflow-y-auto">
        {draft.values.slice(start, start + 50).map((entry, row) => {
          const index = start + row;
          return (
            <div key={index} className="space-y-1 rounded border p-1.5">
              <Input
                aria-label={`${t("conversion.code")} ${index + 1}`}
                placeholder={t("conversion.code")}
                value={entry.value}
                disabled={pending}
                onChange={(event) => update(index, "value", event.target.value)}
              />
              <Input
                aria-label={`${t("conversion.label")} ${index + 1}`}
                placeholder={t("conversion.label")}
                value={entry.label}
                disabled={pending}
                onChange={(event) => update(index, "label", event.target.value)}
              />
              <div className="flex gap-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  disabled={pending || index === 0}
                  onClick={() => move(index, -1)}
                >
                  {t("conversion.moveUp")}
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  disabled={pending || index === draft.values.length - 1}
                  onClick={() => move(index, 1)}
                >
                  {t("conversion.moveDown")}
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  disabled={pending}
                  onClick={() => remove(index)}
                >
                  {t("conversion.removeValue")}
                </Button>
              </div>
            </div>
          );
        })}
      </div>
      {lastPage > 0 && (
        <div className="flex items-center gap-2 text-xs">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={shownPage === 0}
            onClick={() => setPage(shownPage - 1)}
          >
            {t("conversion.previousValues")}
          </Button>
          <span>
            {shownPage + 1}/{lastPage + 1}
          </span>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={shownPage === lastPage}
            onClick={() => setPage(shownPage + 1)}
          >
            {t("conversion.nextValues")}
          </Button>
        </div>
      )}
      {draft.values.length === 2 && (
        <label className="block space-y-1 text-xs">
          <span>{t("conversion.positiveValue")}</span>
          <select
            className={detailInlineInputClass}
            disabled={pending}
            value={
              draft.positiveValue === null
                ? ""
                : String(draft.values.findIndex((entry) => entry.value === draft.positiveValue))
            }
            onChange={(event) =>
              setDraft((current) => ({
                ...current,
                positiveValue:
                  event.target.value === ""
                    ? null
                    : current.values[Number(event.target.value)].value,
              }))
            }
          >
            <option value="">{t("conversion.unspecified")}</option>
            {draft.values.map((entry, index) => (
              <option key={index} value={index}>
                {entry.label || entry.value || "∅"}
              </option>
            ))}
          </select>
        </label>
      )}
      {!unique && (
        <p role="alert" className="text-xs text-destructive">
          {t("conversion.duplicateValues")}
        </p>
      )}
      <div className="flex flex-wrap gap-1">
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={pending || draft.values.length >= 65536}
          onClick={() => {
            setPage(Math.floor(draft.values.length / 50));
            setDraft((current) => ({
              ...current,
              values: [...current.values, { value: "", label: "" }],
            }));
          }}
        >
          {t("conversion.addValue")}
        </Button>
        <Button
          type="button"
          size="sm"
          disabled={pending || !unique}
          onClick={() =>
            onCommit(draft, { onRejected: () => setDraft(readDomain(latestProjection.current)) })
          }
        >
          {t("conversion.applyDomain")}
        </Button>
      </div>
    </div>
  );
}
