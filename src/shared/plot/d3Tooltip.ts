import type { ChartThemeColors } from '@/shared/theme/chartTheme';

/** d3 selection subset used for `.on()` handlers without `as unknown` casts */
export type D3Onable<GElement extends Element = Element, Datum = unknown> = {
  on(
    typenames: string,
    listener: ((this: GElement, event: MouseEvent, datum: Datum) => void) | null,
  ): D3Onable<GElement, Datum>;
};

export interface TooltipOffset {
  x: number;
  y: number;
}

export interface AnchorTooltipPositionInput {
  containerWidth: number;
  anchorLeft: number;
  anchorTop: number;
  anchorWidth: number;
  anchorHeight: number;
  tooltipWidth: number;
  tooltipHeight: number;
  padding?: number;
}

/** Anchor tooltip above target when space allows, otherwise below; clamp horizontal center. */
export function computeAnchorTooltipPosition(input: AnchorTooltipPositionInput): {
  left: number;
  top: number;
} {
  const padding = input.padding ?? 6;
  let left =
    input.anchorLeft + input.anchorWidth / 2 - input.tooltipWidth / 2;
  left = Math.max(4, Math.min(left, input.containerWidth - input.tooltipWidth - 4));
  const above = input.anchorTop - input.tooltipHeight - padding;
  const below = input.anchorTop + input.anchorHeight + padding;
  return {
    left,
    top: above > 0 ? above : below,
  };
}

export interface PointerTooltipPositionInput {
  containerWidth: number;
  containerHeight: number;
  pointerLeft: number;
  pointerTop: number;
  tooltipWidth: number;
  tooltipHeight: number;
  offset?: TooltipOffset;
  centerX?: boolean;
}

/** Cursor tooltip with optional horizontal centering and above/below flip. */
export function computePointerTooltipPosition(input: PointerTooltipPositionInput): {
  left: number;
  top: number;
} {
  const offset = input.offset ?? { x: 8, y: -36 };
  const left = input.centerX
    ? Math.max(
        4,
        Math.min(
          input.pointerLeft - input.tooltipWidth / 2,
          input.containerWidth - input.tooltipWidth - 4,
        ),
      )
    : input.pointerLeft + offset.x;
  if (input.centerX) {
    const above = input.pointerTop - input.tooltipHeight - 8;
    const below = input.pointerTop + 8;
    return { left, top: above > 0 ? above : below };
  }
  return {
    left,
    top: input.pointerTop + offset.y,
  };
}

export function escapeTooltipHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

export function tooltipRichBlock(html: string, theme: ChartThemeColors): string {
  return `<div style="font-size:11px;line-height:1.6;color:${theme.tooltipFg}">${html}</div>`;
}

export function tooltipMutedLine(text: string, theme: ChartThemeColors, size = 10): string {
  return `<div style="font-size:${size}px;color:${theme.tooltipMuted}">${escapeTooltipHtml(text)}</div>`;
}

export function tooltipStrongLine(
  text: string,
  theme: ChartThemeColors,
  options?: { size?: number; color?: string },
): string {
  const size = options?.size ?? 12;
  const color = options?.color ?? theme.tooltipFg;
  return `<div style="font-size:${size}px;font-weight:600;color:${color}">${escapeTooltipHtml(text)}</div>`;
}

export function tooltipTickLine(text: string, theme: ChartThemeColors, size = 10): string {
  return `<div style="font-size:${size}px;color:${theme.tick}">${escapeTooltipHtml(text)}</div>`;
}

export function tooltipTwoLine(
  theme: ChartThemeColors,
  muted: string,
  value: string,
  accent?: string,
): string {
  return (
    tooltipMutedLine(muted, theme) +
    `<div style="font-size:11px;font-weight:600;color:${accent ?? theme.tooltipFg}">${escapeTooltipHtml(value)}</div>`
  );
}

export class PlotTooltipController {
  constructor(
    private readonly tooltipEl: HTMLElement | null,
    private readonly containerEl: HTMLElement | null,
  ) {}

  show(html: string): void {
    if (!this.tooltipEl) return;
    this.tooltipEl.style.opacity = '1';
    this.tooltipEl.innerHTML = html;
  }

  hide(): void {
    if (!this.tooltipEl) return;
    this.tooltipEl.style.opacity = '0';
  }

  moveToCursor(event: MouseEvent, offset: TooltipOffset = { x: 8, y: -36 }): void {
    if (!this.tooltipEl || !this.containerEl) return;
    const containerRect = this.containerEl.getBoundingClientRect();
    const { left, top } = computePointerTooltipPosition({
      containerWidth: containerRect.width,
      containerHeight: containerRect.height,
      pointerLeft: event.clientX - containerRect.left,
      pointerTop: event.clientY - containerRect.top,
      tooltipWidth: this.tooltipEl.offsetWidth,
      tooltipHeight: this.tooltipEl.offsetHeight,
      offset,
    });
    this.tooltipEl.style.left = `${left}px`;
    this.tooltipEl.style.top = `${top}px`;
  }

  moveToPointerCentered(event: MouseEvent): void {
    if (!this.tooltipEl || !this.containerEl) return;
    const containerRect = this.containerEl.getBoundingClientRect();
    const { left, top } = computePointerTooltipPosition({
      containerWidth: containerRect.width,
      containerHeight: containerRect.height,
      pointerLeft: event.clientX - containerRect.left,
      pointerTop: event.clientY - containerRect.top,
      tooltipWidth: this.tooltipEl.offsetWidth,
      tooltipHeight: this.tooltipEl.offsetHeight,
      centerX: true,
    });
    this.tooltipEl.style.left = `${left}px`;
    this.tooltipEl.style.top = `${top}px`;
  }

  moveToAnchor(anchor: DOMRect, padding = 6): void {
    if (!this.tooltipEl || !this.containerEl) return;
    const containerRect = this.containerEl.getBoundingClientRect();
    const { left, top } = computeAnchorTooltipPosition({
      containerWidth: containerRect.width,
      anchorLeft: anchor.left - containerRect.left,
      anchorTop: anchor.top - containerRect.top,
      anchorWidth: anchor.width,
      anchorHeight: anchor.height,
      tooltipWidth: this.tooltipEl.offsetWidth,
      tooltipHeight: this.tooltipEl.offsetHeight,
      padding,
    });
    this.tooltipEl.style.left = `${left}px`;
    this.tooltipEl.style.top = `${top}px`;
  }
}

export interface AttachHoverTooltipConfig<GElement extends Element, Datum> {
  tooltip: PlotTooltipController;
  getHtml: (datum: Datum, element: GElement) => string;
  position?: 'cursor' | 'anchor';
  cursorOffset?: TooltipOffset;
  onEnter?: (element: GElement, datum: Datum, event: MouseEvent) => void;
  onMove?: (element: GElement, datum: Datum, event: MouseEvent) => void;
  onLeave?: (element: GElement, datum: Datum) => void;
}

/** Standard enter / move / leave tooltip wiring for bar, cell, point marks. */
export function attachHoverTooltip<GElement extends Element, Datum>(
  selection: D3Onable<GElement, Datum>,
  config: AttachHoverTooltipConfig<GElement, Datum>,
): void {
  const position = config.position ?? 'cursor';

  selection
    .on('mouseenter', function (this: GElement, event: MouseEvent, datum: Datum) {
      config.onEnter?.(this, datum, event);
      config.tooltip.show(config.getHtml(datum, this));
      if (position === 'anchor') {
        config.tooltip.moveToAnchor(this.getBoundingClientRect());
      } else {
        config.tooltip.moveToCursor(event, config.cursorOffset);
      }
    })
    .on('mousemove', function (this: GElement, event: MouseEvent, datum: Datum) {
      config.onMove?.(this, datum, event);
      if (position === 'cursor') {
        config.tooltip.moveToCursor(event, config.cursorOffset);
      }
    })
    .on('mouseleave', function (this: GElement, _event: MouseEvent, datum: Datum) {
      config.onLeave?.(this, datum);
      config.tooltip.hide();
    });
}

export interface AttachOverlayCursorTooltipConfig {
  tooltip: PlotTooltipController;
  onMove: (event: MouseEvent) => string;
  onLeave?: () => void;
  centered?: boolean;
}

/** Transparent plot overlay: tooltip follows pointer (IRF-style). */
export function attachOverlayCursorTooltip(
  selection: D3Onable<SVGRectElement, unknown>,
  config: AttachOverlayCursorTooltipConfig,
): void {
  selection
    .on('mousemove', function (event: MouseEvent) {
      config.tooltip.show(config.onMove(event));
      if (config.centered) {
        config.tooltip.moveToPointerCentered(event);
      } else {
        config.tooltip.moveToCursor(event);
      }
    })
    .on('mouseleave', () => {
      config.onLeave?.();
      config.tooltip.hide();
    });
}
