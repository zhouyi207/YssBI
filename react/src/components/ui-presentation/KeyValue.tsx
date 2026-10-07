import type { UiMetric } from "@/shared/types/domain/uiData";
import { formatValue, valueClass } from "./formatValue";

export function KeyValue({ items }: { items: readonly UiMetric[] }) {
  return (
    <dl className="mb-2 grid grid-cols-1 gap-px overflow-hidden rounded-lg border border-border bg-border sm:grid-cols-2">
      {items.map((item) => (
        <div key={item.id} className="flex justify-between gap-4 bg-card px-4 py-2.5 text-xs">
          <dt className="text-muted-foreground">{item.label}</dt>
          <dd className={`font-mono font-medium ${valueClass(item.value, item.format)}`}>
            {formatValue(item.value, item.format)}
          </dd>
        </div>
      ))}
    </dl>
  );
}
