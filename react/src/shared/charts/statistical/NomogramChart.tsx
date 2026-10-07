import { cn } from "@/lib/utils";
import type { ChartModel } from "../ChartModel";
import { useChartTheme } from "../core/theme";
import type { ChartSurfaceVariant } from "../core/types";
import { useChartContainerSize } from "../core/useChartContainerSize";

/** Tick locations and labels are computed by Rust; this component only lays out axes. */
export function NomogramChart({
  model,
  surface,
}: {
  model: Extract<ChartModel, { kind: "nomogram" }>;
  surface: ChartSurfaceVariant;
}) {
  const { containerRef, size } = useChartContainerSize();
  const { colors, series } = useChartTheme();
  const width = Math.max(640, size.width);
  const height = model.axes.length * 82 + 24;
  const left = 40;
  const span = width - 80;
  return (
    <div
      ref={containerRef}
      className={cn(
        "h-full min-h-0 w-full overflow-auto",
        surface === "card" && "rounded-lg border border-border bg-card",
      )}
    >
      <svg
        width={width}
        height={height}
        role="img"
        aria-label="Cox nomogram"
        style={{ background: colors.canvas }}
      >
        <title>Cox nomogram</title>
        {model.axes.map((axis, row) => {
          const y = row * 82 + 50;
          const sorted = [...axis.ticks].sort((a, b) => a.position - b.position);
          let previousLabel = -Infinity;
          return (
            <g key={row}>
              <text x={left} y={y - 18} fill={colors.label} fontSize={13}>
                <title>{axis.label}</title>
                {axis.label.length > 85 ? `${axis.label.slice(0, 82)}…` : axis.label}
              </text>
              <line
                x1={left + sorted[0].position * span}
                x2={left + sorted[sorted.length - 1].position * span}
                y1={y}
                y2={y}
                stroke={colors.axis}
              />
              {sorted.map((tick, i) => {
                const x = left + tick.position * span;
                const showLabel = x - previousLabel >= 60;
                if (showLabel) previousLabel = x;
                return (
                  <g key={i}>
                    <title>{`${axis.label}: ${tick.label}`}</title>
                    <line
                      x1={x}
                      x2={x}
                      y1={y - 4}
                      y2={y + 5}
                      stroke={row === 0 ? series.primary : colors.tick}
                    />
                    {showLabel && (
                      <text x={x} y={y + 23} textAnchor="middle" fill={colors.tick} fontSize={12}>
                        {tick.label}
                      </text>
                    )}
                  </g>
                );
              })}
            </g>
          );
        })}
      </svg>
    </div>
  );
}
