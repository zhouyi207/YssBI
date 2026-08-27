import type { ChartThemeColors } from '@/shared/theme/chartTheme';

export type MarkInteractionEvent = MouseEvent | FocusEvent;

/** D3 selection subset used by mark tooltip bindings. */
export type D3Onable<GElement extends Element = Element, Datum = unknown> = {
  attr(
    name: string,
    value:
      | string
      | number
      | null
      | ((this: GElement, datum: Datum) => string | number | null),
  ): D3Onable<GElement, Datum>;
  on(
    typenames: string,
    listener:
      | ((this: GElement, event: MarkInteractionEvent, datum: Datum) => void)
      | null,
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
  pointerLeft: number;
  pointerTop: number;
  offset?: TooltipOffset;
}

/** Cursor tooltip positioned at a stable offset from the pointer. */
export function computePointerTooltipPosition(input: PointerTooltipPositionInput): {
  left: number;
  top: number;
} {
  const offset = input.offset ?? { x: 8, y: -36 };
  return {
    left: input.pointerLeft + offset.x,
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
      pointerLeft: event.clientX - containerRect.left,
      pointerTop: event.clientY - containerRect.top,
      offset,
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

export interface AttachMarkTooltipConfig<GElement extends Element, Datum> {
  tooltip: PlotTooltipController;
  getHtml: (datum: Datum, element: GElement) => string;
  getAriaLabel?: (datum: Datum, element: GElement) => string;
  position?: 'cursor' | 'anchor';
  cursorOffset?: TooltipOffset;
  onEnter?: (
    element: GElement,
    datum: Datum,
    event: MarkInteractionEvent,
  ) => void;
  onMove?: (element: GElement, datum: Datum, event: MouseEvent) => void;
  onLeave?: (element: GElement, datum: Datum) => void;
}

/** Pointer and keyboard tooltip wiring for bar, cell, and point marks. */
export function attachMarkTooltip<GElement extends Element, Datum>(
  selection: D3Onable<GElement, Datum>,
  config: AttachMarkTooltipConfig<GElement, Datum>,
): void {
  const position = config.position ?? 'cursor';

  selection.attr('tabindex', 0);
  if (config.getAriaLabel) {
    selection.attr('aria-label', function (this: GElement, datum: Datum) {
      return config.getAriaLabel?.(datum, this) ?? null;
    });
  }

  selection
    .on('mouseenter', function (this: GElement, event: MarkInteractionEvent, datum: Datum) {
      config.onEnter?.(this, datum, event);
      config.tooltip.show(config.getHtml(datum, this));
      if (position === 'anchor') {
        config.tooltip.moveToAnchor(this.getBoundingClientRect());
      } else {
        config.tooltip.moveToCursor(event as MouseEvent, config.cursorOffset);
      }
    })
    .on('mousemove', function (this: GElement, event: MarkInteractionEvent, datum: Datum) {
      const mouseEvent = event as MouseEvent;
      config.onMove?.(this, datum, mouseEvent);
      if (position === 'cursor') {
        config.tooltip.moveToCursor(mouseEvent, config.cursorOffset);
      }
    })
    .on('mouseleave', function (this: GElement, _event: MarkInteractionEvent, datum: Datum) {
      config.onLeave?.(this, datum);
      config.tooltip.hide();
    })
    .on('focus', function (this: GElement, event: MarkInteractionEvent, datum: Datum) {
      config.onEnter?.(this, datum, event);
      config.tooltip.show(config.getHtml(datum, this));
      config.tooltip.moveToAnchor(this.getBoundingClientRect());
    })
    .on('blur', function (this: GElement, _event: MarkInteractionEvent, datum: Datum) {
      config.onLeave?.(this, datum);
      config.tooltip.hide();
    });
}
