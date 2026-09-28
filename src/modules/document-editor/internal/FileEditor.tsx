import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import type { FileSnapshot } from "@/shared/types/domain/fileDocument";
import type { FileResourceKind } from "@/shared/types/domain/resource";
import type { createFileProjectionStore } from "@/features/core/resource/fileProjectionStore";
import { resourceKey, useResourceStore } from "@/features/core/resource";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
export function FileEditor<S extends FileSnapshot<FileResourceKind, unknown>>({
  resourceRef,
  resourceKind,
  store,
  actions,
  renderContent,
}: {
  resourceRef: string;
  resourceKind: S["kind"];
  store: ReturnType<typeof createFileProjectionStore<S>>["store"];
  actions: {
    load(path: string): Promise<S>;
    save(path: string): Promise<boolean>;
  };
  renderContent(snapshot: S, reportError: (error: unknown) => void): ReactNode;
}) {
  const { t } = useTranslation();
  const snapshot = store((state) => state.documents[resourceRef]);
  const revision = useResourceStore(
    (state) => state.resources[resourceKey({ id: resourceRef, kind: resourceKind })]?.revision,
  );
  const [error, setError] = useState<string | null>(null);
  const reportError = useCallback(
    (error: unknown) => setError(formatInlineUserError(error, t)),
    [t],
  );
  useEffect(() => {
    const loaded = store.getState().documents[resourceRef];
    if (loaded && (revision === undefined || loaded.version.revision === revision)) return;
    let current = true;
    void actions.load(resourceRef).catch((error) => {
      if (current) reportError(error);
    });
    return () => {
      current = false;
    };
  }, [resourceRef, revision, reportError, actions, store]);
  const save = () => {
    setError(null);
    void actions.save(resourceRef).catch(reportError);
  };
  return (
    <section
      className="flex h-full min-h-0 flex-col bg-background"
      onKeyDown={(event) => {
        if (
          !event.defaultPrevented &&
          !event.shiftKey &&
          (event.ctrlKey || event.metaKey) &&
          event.key.toLowerCase() === "s"
        ) {
          event.preventDefault();
          event.stopPropagation();
          save();
        }
      }}
    >
      {error && (
        <div
          role="alert"
          className="flex items-center gap-2 border-b px-3 py-2 text-xs text-destructive"
        >
          {error}
          <Button variant="ghost" size="sm" onClick={() => setError(null)}>
            {t("common.close")}
          </Button>
        </div>
      )}
      {!snapshot ? (
        <p className="p-5 text-sm text-muted-foreground">{t("common.loading")}</p>
      ) : (
        renderContent(snapshot, reportError)
      )}
    </section>
  );
}
