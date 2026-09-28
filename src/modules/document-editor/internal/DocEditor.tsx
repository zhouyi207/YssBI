import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { MarkdownRenderer } from "@/shared/ui/MarkdownRenderer";
import { markdownProseClass } from "@/shared/ui/markdownProseClass";
import type { EditorPanelScope } from "@/modules/workbench/public";
import type { DocSnapshot } from "@/shared/types/domain/doc";
import { useDocProjectionStore } from "@/features/core/resource/docProjectionStore";
import { docActions } from "@/features/application/resource/docActions";
import { useFileTextInput } from "@/features/application/resource/useFileTextInput";
import { FileEditor } from "./FileEditor";
function MarkdownEditor({
  snapshot,
  reportError,
}: {
  snapshot: DocSnapshot;
  reportError(error: unknown): void;
}) {
  const { t } = useTranslation();
  const [isPreview, setIsPreview] = useState(false);
  const input = useFileTextInput(
    snapshot,
    snapshot.content,
    (markdown) => ({ op: "set_markdown", markdown }),
    docActions,
    "markdown",
  );
  return (
    <div className="relative isolate flex min-h-0 flex-1 flex-col">
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="absolute right-3 top-2 z-10 bg-background shadow-sm dark:bg-background"
        onClick={() => setIsPreview((current) => !current)}
      >
        {t(isPreview ? "documents.edit" : "documents.preview")}
      </Button>
      <textarea
        hidden={isPreview}
        aria-label={t("documents.markdown")}
        className="min-h-0 w-full flex-1 resize-none bg-background p-5 pt-2 text-sm outline-none"
        spellCheck={false}
        value={input.value}
        onChange={(event) => input.change(event.target.value)}
        onBlur={() => void input.flush().catch(reportError)}
      />
      {isPreview && (
        <article
          aria-label={t("documents.preview")}
          tabIndex={0}
          className="min-h-0 min-w-0 flex-1 overflow-auto px-[clamp(1.5rem,4%,5rem)] py-10"
        >
          <div className={`${markdownProseClass} mx-auto w-full max-w-full prose-base`}>
            <MarkdownRenderer markdown={input.value} />
          </div>
        </article>
      )}
    </div>
  );
}

export function DocFileEditor(scope: EditorPanelScope<"doc">) {
  return (
    <FileEditor
      {...scope}
      store={useDocProjectionStore}
      actions={docActions}
      renderContent={(snapshot, reportError) => (
        <MarkdownEditor
          key={snapshot.version.sessionId}
          snapshot={snapshot}
          reportError={reportError}
        />
      )}
    />
  );
}
