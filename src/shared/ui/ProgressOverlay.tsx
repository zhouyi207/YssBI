import { useTranslation } from "react-i18next";
import { VscClose } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import type { ProgressState } from "@/shared/types/ui";

export interface ProgressOverlayProps {
  progress: ProgressState;
  onCancel: () => void;
}

/**
 * 全局进度蒙层（项目选择页打开/新建/扫描/清理等共用）。
 *
 * - `progress.percent` 为 0~1 时显示确定型进度条；未提供时显示不确定型滑动条。
 * - `progress.cancelable` 为 true 时显示关闭按钮。
 */
export function ProgressOverlay({ progress, onCancel }: ProgressOverlayProps) {
  const { t } = useTranslation();
  const indeterminate = progress.percent === undefined;
  const pct = indeterminate
    ? 0
    : Math.min(100, Math.max(0, (progress.percent ?? 0) * 100));
  const showCloseButton = progress.cancelable === true;

  return (
    <div
      role="status"
      aria-live="polite"
      aria-busy="true"
      className="fixed inset-0 z-[1000] flex items-center justify-center bg-background/70 backdrop-blur-sm"
    >
      <Card className="relative w-[min(420px,90vw)] border-border/60 shadow-lg">
        {showCloseButton && (
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className="absolute right-3 top-3 z-10"
            onClick={onCancel}
            aria-label={t("common.close")}
          >
            <VscClose size={16} />
          </Button>
        )}
        <CardContent className="px-6 py-5">
          <div className={`mb-3 flex items-baseline justify-between gap-3 ${showCloseButton ? "pr-8" : ""}`}>
            <h3 className="truncate text-sm font-medium text-foreground">
              {progress.stage}
            </h3>
            {!indeterminate && (
              <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
                {Math.round(pct)}%
              </span>
            )}
          </div>

          {indeterminate ? (
            <div className="relative h-2 w-full overflow-hidden rounded-full bg-muted">
              <div className="loading-indeterminate-bar" />
            </div>
          ) : (
            <Progress value={progress.percent ?? 0} max={1} />
          )}

          {progress.detail && (
            <p className="mt-2 truncate text-xs text-muted-foreground">
              {progress.detail}
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
