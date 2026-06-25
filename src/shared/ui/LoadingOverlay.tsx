import { Card, CardContent } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import type { ProgressState } from "@/shared/types/ui";

/**
 * 全局加载蒙层。
 *
 * - `progress.percent` 为 0~1 时显示确定型进度条；未提供时显示
 *   不确定型滑动条（依赖 `loading-indeterminate-bar` CSS 动画）。
 * - 蒙层会拦截所有指针事件，确保加载阶段用户无法触发其他操作。
 */
export const LoadingOverlay = ({ progress }: { progress: ProgressState }) => {
  const indeterminate = progress.percent === undefined;
  const pct = indeterminate
    ? 0
    : Math.min(100, Math.max(0, (progress.percent ?? 0) * 100));

  return (
    <div
      role="status"
      aria-live="polite"
      aria-busy="true"
      className="fixed inset-0 z-[1000] flex items-center justify-center bg-background/70 backdrop-blur-sm"
    >
      <Card className="w-[min(420px,90vw)] border-border/60 shadow-lg">
        <CardContent className="px-6 py-5">
          <div className="mb-3 flex items-baseline justify-between gap-3">
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
};
