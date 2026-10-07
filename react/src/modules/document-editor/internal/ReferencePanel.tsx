import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { VscLinkExternal, VscRefresh } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { openExternalUrlWithDialog } from "@/features/application/window/openExternalUrlWithDialog";
import { parseReferenceUrl } from "@/shared/utils/referenceUrl";
import { loadReferencePdf } from "@/services/platform/referencePdf";

type PdfPreview = { status: "loading" | "failed" } | { status: "ready"; url: string };

export function ReferencePanel({ url, title }: { url: string; title: string }) {
  const { t } = useTranslation();
  const [reload, setReload] = useState(0);
  const [preview, setPreview] = useState<PdfPreview>({ status: "loading" });
  const address = parseReferenceUrl(url);
  const pdf = address !== null && /\.pdf$/i.test(address.pathname);

  useEffect(() => {
    if (!pdf) return;
    const controller = new AbortController();
    let objectUrl: string | undefined;
    setPreview({ status: "loading" });
    void loadReferencePdf(url, controller.signal)
      .then((blob) => {
        if (controller.signal.aborted) return;
        objectUrl = URL.createObjectURL(blob);
        setPreview({ status: "ready", url: objectUrl });
      })
      .catch(() => {
        if (!controller.signal.aborted) setPreview({ status: "failed" });
      });
    return () => {
      controller.abort();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [pdf, url, reload]);

  if (!address) return null;
  const sameOrigin = address.origin === window.location.origin;

  return (
    <section className="flex h-full min-h-0 flex-col" aria-label={title}>
      <div className="flex shrink-0 items-center gap-1 border-b bg-[var(--surface-raised)] px-2 py-1">
        <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground" title={url}>
          {address.hostname}
        </span>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={t("reference.reload")}
          title={t("reference.reload")}
          onClick={() => setReload((value) => value + 1)}
        >
          <VscRefresh aria-hidden />
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={t("reference.openExternal")}
          title={t("reference.openExternal")}
          onClick={() => void openExternalUrlWithDialog(url, t)}
        >
          <VscLinkExternal aria-hidden />
        </Button>
      </div>
      {pdf && preview.status !== "ready" ? (
        <div
          className="flex min-h-0 flex-1 items-center justify-center p-4 text-sm text-muted-foreground"
          role={preview.status === "failed" ? "alert" : "status"}
        >
          {t(preview.status === "failed" ? "reference.loadFailed" : "common.loading")}
        </div>
      ) : (
        <iframe
          key={reload}
          src={pdf && preview.status === "ready" ? preview.url + address.hash : address.href}
          title={title}
          className="min-h-0 w-full flex-1 border-0 bg-white"
          referrerPolicy="no-referrer"
          // Native PDF plugins cannot run in an HTML sandbox. Only validated PDF
          // bytes use a local blob; remote web pages cannot navigate the top frame.
          sandbox={
            pdf
              ? undefined
              : sameOrigin
                ? "allow-forms"
                : "allow-scripts allow-same-origin allow-forms"
          }
        />
      )}
      <p className="shrink-0 border-t px-2 py-1 text-xs text-muted-foreground">
        {t("reference.previewHint")}
      </p>
    </section>
  );
}
