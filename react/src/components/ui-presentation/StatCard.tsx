import { Card, CardContent } from "@/components/ui/card";
import type { UiValueFormat } from "@/shared/types/domain/uiData";
import { formatValue } from "./formatValue";

export function StatCard({
  label,
  value,
  sub,
  format = "text",
}: {
  label: string;
  value: string | number;
  sub?: string;
  format?: UiValueFormat;
}) {
  return (
    <Card className="rounded-lg py-0 shadow-none">
      <CardContent className="px-4 py-3">
        <div className="mb-1 text-[11px] uppercase tracking-wider text-muted-foreground">
          {label}
        </div>
        <div className="font-mono text-sm font-medium text-foreground">
          {formatValue(value, format)}
        </div>
        {sub && <div className="mt-0.5 text-[10px] text-muted-foreground">{sub}</div>}
      </CardContent>
    </Card>
  );
}
