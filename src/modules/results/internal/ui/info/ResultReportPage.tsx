import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { UiPageRenderer, type UiResultBindings } from "@/components/ui-presentation/UiPageRenderer";
import { useUiPage } from "@/features/application/presentation/useUiPage";
import { ResultReadError } from "@/features/application/results/components/ResultReadError";
import type { ResultReference } from "@/shared/types/domain/result";
import type { UiDisplayData } from "@/shared/types/domain/uiData";
import { ResultPageLayoutControls } from "./ResultPageLayoutControls";

export function ResultReportPage({
  reference,
  data,
  bindings,
  children,
}: {
  reference: ResultReference;
  data: Readonly<Record<string, UiDisplayData>>;
  bindings: UiResultBindings;
  children?: ReactNode;
}) {
  const { t } = useTranslation();
  const presentation = useUiPage(reference);
  return (
    <div className="mx-auto w-full max-w-[1100px] space-y-4 p-6">
      {children}
      {presentation.error && (
        <ResultReadError error={presentation.error} onRetry={() => void presentation.reload()} />
      )}
      {!presentation.page && !presentation.error && (
        <p className="text-sm text-muted-foreground">{t("common.loading")}</p>
      )}
      {presentation.page && (
        <>
          <ResultPageLayoutControls
            spec={presentation.page.spec}
            busy={presentation.busy}
            onAction={presentation.act}
          />
          <UiPageRenderer
            spec={presentation.page.spec}
            data={data}
            results={bindings}
            disabled={presentation.busy}
            onActivate={(id) => void presentation.activate(id)}
          />
        </>
      )}
    </div>
  );
}
