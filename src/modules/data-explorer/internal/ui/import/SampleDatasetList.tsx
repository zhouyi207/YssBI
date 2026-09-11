import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { VscDatabase, VscLoading } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import {
  sampleDatasetFailure,
  useSampleDatasets,
} from "@/features/application/dataManagement/useSampleDatasets";

export function SampleDatasetList({
  onImport,
  onImported,
  onBusyChange,
}: {
  onImport: (id: string, version: number) => Promise<void>;
  onImported: () => void;
  onBusyChange: (busy: boolean) => void;
}) {
  const { t, i18n } = useTranslation();
  const catalog = useSampleDatasets();
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [importError, setImportError] = useState<ReturnType<typeof sampleDatasetFailure> | null>(
    null,
  );
  const inFlight = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  async function importSample(id: string, version: number) {
    if (inFlight.current) return;
    inFlight.current = true;
    setPendingId(id);
    setImportError(null);
    onBusyChange(true);
    try {
      await onImport(id, version);
      if (mounted.current) onImported();
    } catch (error) {
      if (mounted.current) setImportError(sampleDatasetFailure(error, "import_sample_dataset"));
    } finally {
      inFlight.current = false;
      if (mounted.current) {
        setPendingId(null);
        onBusyChange(false);
      }
    }
  }

  const error = importError ?? catalog.error;
  const numbers = new Intl.NumberFormat(i18n.language);
  return (
    <div aria-busy={catalog.loading || pendingId !== null}>
      {error && (
        <div
          role="alert"
          className="mt-4 rounded-sm border border-destructive/30 bg-destructive/5 p-3 text-xs leading-relaxed"
        >
          <p>
            {t(`importModal.samples.errors.${error.code}`, {
              defaultValue: t("importModal.samples.failed"),
            })}
          </p>
          {error.incidentId && (
            <p className="mt-1 text-muted-foreground">
              {t("common.incidentId")}: {error.incidentId}
            </p>
          )}
          {catalog.error && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="mt-2"
              onClick={catalog.retry}
            >
              {t("importModal.samples.retry")}
            </Button>
          )}
        </div>
      )}
      {catalog.loading ? (
        <p role="status" className="flex items-center gap-2 py-6 text-xs text-muted-foreground">
          <VscLoading
            aria-hidden="true"
            className="size-4 animate-spin motion-reduce:animate-none"
          />
          {t("importModal.samples.loading")}
        </p>
      ) : !catalog.error && catalog.samples.length === 0 ? (
        <p className="py-6 text-xs text-muted-foreground">{t("importModal.samples.empty")}</p>
      ) : (
        catalog.samples.map((sample) => {
          const pending = pendingId === sample.id;
          const size =
            sample.byteSize >= 1_000_000
              ? `${(sample.byteSize / 1_000_000).toLocaleString(i18n.language, { maximumFractionDigits: 1 })} MB`
              : `${Math.ceil(sample.byteSize / 1_000).toLocaleString(i18n.language)} KB`;
          return (
            <div key={sample.id} className="flex items-start gap-3 border-b border-border py-5">
              <VscDatabase
                aria-hidden="true"
                className="mt-0.5 size-4 shrink-0 text-muted-foreground"
              />
              <div className="min-w-0 flex-1">
                <h3 className="text-[13px] font-medium leading-normal">
                  {t(`importModal.samples.datasets.${sample.id}.name`, {
                    defaultValue: sample.name,
                  })}
                </h3>
                <p className="mt-1 text-xs leading-[1.65] text-muted-foreground">
                  {t(`importModal.samples.datasets.${sample.id}.description`, {
                    defaultValue: t("importModal.samples.defaultDescription"),
                  })}
                </p>
                <p className="mt-2 text-[11px] text-muted-foreground">
                  {t("importModal.samples.dimensions", {
                    rows: numbers.format(sample.rowCount),
                    columns: numbers.format(sample.columnCount),
                  })}{" "}
                  · {size}
                </p>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="mt-3 gap-2"
                  disabled={pendingId !== null}
                  onClick={() => {
                    void importSample(sample.id, sample.version);
                  }}
                >
                  {pending && (
                    <VscLoading
                      aria-hidden="true"
                      className="size-3.5 animate-spin motion-reduce:animate-none"
                    />
                  )}
                  {t(pending ? "importModal.samples.importing" : "importModal.samples.import")}
                </Button>
              </div>
            </div>
          );
        })
      )}
      {pendingId && (
        <p role="status" className="mt-3 text-xs text-muted-foreground">
          {t("importModal.samples.importing")}
        </p>
      )}
    </div>
  );
}
