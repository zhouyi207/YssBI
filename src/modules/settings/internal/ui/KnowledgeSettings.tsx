import { useEffect, useId, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useProjectKnowledge } from "@/features/application/assistant/useProjectKnowledge";
import { openAssistantResource } from "@/features/application/assistant/assistantResourceActions";
import { SettingsField } from "./SettingsField";
import { SettingsPage } from "./SettingsPage";

export function KnowledgeSettings({ notice }: { notice?: ReactNode }) {
  const { t, i18n } = useTranslation();
  const id = useId();
  const knowledge = useProjectKnowledge();
  const [selected, setSelected] = useState("");
  const [openFailed, setOpenFailed] = useState(false);
  const clock = useMemo(
    () => new Intl.DateTimeFormat(i18n.language, { dateStyle: "short", timeStyle: "short" }),
    [i18n.language],
  );
  useEffect(() => {
    setSelected("");
    setOpenFailed(false);
  }, [knowledge.projectInstanceId]);
  const busy = knowledge.loading || knowledge.pending !== null;
  const available = knowledge.documents.filter(
    (doc) => !knowledge.sources.some((source) => source.path === doc.id),
  );
  const selection = available.some((doc) => doc.id === selected) ? selected : "";
  const failed = !!knowledge.projectInstanceId && (knowledge.failure !== null || openFailed);
  return (
    <SettingsPage
      breadcrumbs={[{ label: t("settings.knowledge.title") }]}
      notice={
        notice || failed ? (
          <>
            {notice}
            {failed && (
              <Alert variant="destructive">
                <AlertDescription className="flex flex-wrap items-center justify-between gap-3">
                  <span className="min-w-0 flex-1">
                    {t(
                      knowledge.failure === "load"
                        ? "settings.knowledge.loadFailed"
                        : "settings.knowledge.failed",
                    )}
                  </span>
                  {knowledge.failure === "load" && (
                    <Button
                      type="button"
                      variant="outline"
                      className="h-8 shrink-0 px-3"
                      disabled={busy}
                      onClick={knowledge.reload}
                    >
                      {t("common.retry")}
                    </Button>
                  )}
                </AlertDescription>
              </Alert>
            )}
          </>
        ) : null
      }
    >
      {!knowledge.projectInstanceId ? (
        <p className="text-xs">{t("settings.knowledge.noProject")}</p>
      ) : (
        <>
          <SettingsField
            htmlFor={`${id}-document`}
            label={t("settings.knowledge.document")}
            description={t("settings.knowledge.description")}
          >
            <div className="flex flex-wrap items-center gap-2">
              <Select
                disabled={busy || !available.length}
                value={selection}
                onValueChange={setSelected}
              >
                <SelectTrigger
                  id={`${id}-document`}
                  aria-describedby={`${id}-document-description`}
                  className="min-w-40 flex-1"
                >
                  <SelectValue
                    placeholder={t(
                      available.length
                        ? "settings.knowledge.choose"
                        : "settings.knowledge.noDocuments",
                    )}
                  />
                </SelectTrigger>
                <SelectContent>
                  {available.map((doc) => (
                    <SelectItem key={doc.id} value={doc.id}>
                      {doc.name} · {doc.id}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Button
                type="button"
                className="h-9 px-4"
                disabled={busy || !selection}
                onClick={() => void knowledge.rebuild(selection)}
              >
                {t("settings.knowledge.add")}
              </Button>
            </div>
          </SettingsField>
          {knowledge.loading && (
            <p role="status" className="text-xs">
              {t("settings.knowledge.loading")}
            </p>
          )}
          {knowledge.pending && (
            <p role="status" className="text-xs">
              {t("settings.knowledge.updating")}
            </p>
          )}
          {!knowledge.loading && knowledge.failure !== "load" && !knowledge.sources.length && (
            <p className="text-xs text-muted-foreground">{t("settings.knowledge.empty")}</p>
          )}
          <div className="space-y-2">
            {knowledge.sources.map((source) => (
              <div
                key={source.sourceId}
                className="space-y-3 rounded-lg border border-border bg-card p-4"
              >
                <div className="min-w-0">
                  <p className="truncate text-sm font-medium">{source.title}</p>
                  <p className="wrap-anywhere text-xs text-muted-foreground">{source.path}</p>
                </div>
                <p className="text-xs">{t(`settings.knowledge.status.${source.status}`)}</p>
                <p className="text-xs text-muted-foreground">
                  {t("settings.knowledge.updated", { value: clock.format(source.updatedAt) })}
                </p>
                <div className="flex flex-wrap gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    className="h-8 px-3"
                    disabled={busy || source.status === "unavailable"}
                    onClick={async () => {
                      setOpenFailed(false);
                      try {
                        await openAssistantResource({ kind: "doc", id: source.path });
                      } catch {
                        setOpenFailed(true);
                      }
                    }}
                  >
                    {t("settings.knowledge.open")}
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    className="h-8 px-3"
                    disabled={busy || source.status === "unavailable"}
                    onClick={() => void knowledge.rebuild(source.path)}
                  >
                    {t("settings.knowledge.rebuild")}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    className="h-8 px-3 text-muted-foreground hover:text-destructive"
                    disabled={busy}
                    onClick={() => void knowledge.remove(source.sourceId)}
                  >
                    {t("settings.knowledge.remove")}
                  </Button>
                </div>
              </div>
            ))}
          </div>
        </>
      )}
      <p className="text-xs leading-relaxed text-muted-foreground">
        {t("settings.knowledge.builtin")}
      </p>
    </SettingsPage>
  );
}
