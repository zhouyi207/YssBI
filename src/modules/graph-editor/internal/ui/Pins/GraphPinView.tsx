import type { CSSProperties, MouseEventHandler, PointerEventHandler, ReactNode } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { PinRenderStyle, PinVisualSpec } from "@/shared/types/domain/pinVisual";

export interface GraphPinConnectionFeedbackViewModel {
  kind: "append" | "replace" | "invalid";
  invalidReason?: string;
}

export function pinConnectionFeedbackAttributes(
  feedback: GraphPinConnectionFeedbackViewModel | null,
) {
  if (!feedback) return {};
  return feedback.kind === "invalid"
    ? {
        "data-connection-feedback": feedback.kind,
        "data-connection-invalid-reason": feedback.invalidReason,
      }
    : { "data-connection-feedback": feedback.kind };
}

export function pinConnectionFeedbackClass(
  feedback: GraphPinConnectionFeedbackViewModel | null,
): string {
  if (!feedback) return "";
  if (feedback.kind === "invalid") return "ring-2 ring-red-500/90";
  return feedback.kind === "replace" ? "ring-2 ring-amber-500/90" : "ring-2 ring-emerald-500/90";
}

export interface GraphPinViewProps {
  id: string;
  name: string;
  direction: "input" | "output";
  isConnected: boolean;
  contextMenuOpen: boolean;
  validationWarning?: string;
  dragStyle?: CSSProperties;
  connectionFeedback: GraphPinConnectionFeedbackViewModel | null;
  visualSpec: PinVisualSpec;
  renderStyle: PinRenderStyle;
  baseColor: string;
  shouldPulse: boolean;
  tooltip: string;
  inputSlot?: ReactNode;
  contextMenuSlot?: ReactNode;
  onContextMenu: MouseEventHandler<HTMLDivElement>;
  onPointerDown?: PointerEventHandler<HTMLDivElement>;
  onClick?: MouseEventHandler<HTMLDivElement>;
}

function GraphPinShape({
  visualSpec,
  renderStyle,
  shouldPulse,
  baseColor,
}: Pick<GraphPinViewProps, "visualSpec" | "renderStyle" | "shouldPulse" | "baseColor">) {
  const { fill, stroke, strokeWidth } = renderStyle;
  const dashed = visualSpec.dashedStroke && !shouldPulse ? { strokeDasharray: "2 2" } : {};
  const pulseStrokeProps = shouldPulse
    ? {
        fill: "none" as const,
        stroke: baseColor,
        strokeWidth: 2.5,
        strokeDasharray:
          visualSpec.shape === "exec" ? "6 24" : visualSpec.shape === "gridRect" ? "8 28" : "7 21",
        className: "pin-flow-stroke",
        filter: "url(#pinGlow)",
      }
    : null;

  switch (visualSpec.shape) {
    case "exec":
      return (
        <>
          <path
            d="M2 2 L7 2 L11 6 L7 10 L2 10 Z"
            fill={fill}
            stroke={stroke}
            strokeWidth={strokeWidth}
            strokeLinejoin="miter"
            {...dashed}
          />
          {pulseStrokeProps ? (
            <path d="M2 2 L7 2 L11 6 L7 10 L2 10 Z" strokeLinejoin="miter" {...pulseStrokeProps} />
          ) : null}
        </>
      );
    case "gridRect":
      return (
        <>
          <g>
            <rect
              x="1.5"
              y="1.5"
              width="9"
              height="9"
              rx="1"
              fill={fill}
              stroke={stroke}
              strokeWidth={strokeWidth}
              {...dashed}
            />
            <line x1="1.5" y1="4.5" x2="10.5" y2="4.5" stroke={stroke} strokeWidth="0.8" />
            <line x1="5" y1="1.5" x2="5" y2="10.5" stroke={stroke} strokeWidth="0.8" />
          </g>
          {pulseStrokeProps ? (
            <rect x="1.5" y="1.5" width="9" height="9" rx="1" {...pulseStrokeProps} />
          ) : null}
        </>
      );
    case "roundedRect":
      return (
        <>
          <rect
            x="2"
            y="2"
            width="8"
            height="8"
            rx="1.5"
            fill={fill}
            stroke={stroke}
            strokeWidth={strokeWidth}
            {...dashed}
          />
          {pulseStrokeProps ? (
            <rect x="2" y="2" width="8" height="8" rx="1.5" {...pulseStrokeProps} />
          ) : null}
        </>
      );
    case "diamond":
      return (
        <>
          <polygon
            points="6,1 11,6 6,11 1,6"
            fill={fill}
            stroke={stroke}
            strokeWidth={strokeWidth}
            strokeLinejoin="miter"
            {...dashed}
          />
          {pulseStrokeProps ? (
            <polygon points="6,1 11,6 6,11 1,6" strokeLinejoin="miter" {...pulseStrokeProps} />
          ) : null}
        </>
      );
    case "hexagon":
      return (
        <>
          <polygon
            points="6,0.5 10.8,3.25 10.8,8.75 6,11.5 1.2,8.75 1.2,3.25"
            fill={fill}
            stroke={stroke}
            strokeWidth={strokeWidth}
            strokeLinejoin="round"
            {...dashed}
          />
          {pulseStrokeProps ? (
            <polygon
              points="6,0.5 10.8,3.25 10.8,8.75 6,11.5 1.2,8.75 1.2,3.25"
              strokeLinejoin="round"
              {...pulseStrokeProps}
            />
          ) : null}
        </>
      );
    default:
      return (
        <>
          <circle
            cx="6"
            cy="6"
            r="4.5"
            fill={fill}
            stroke={stroke}
            strokeWidth={strokeWidth}
            {...dashed}
          />
          {pulseStrokeProps ? <circle cx="6" cy="6" r="4.5" {...pulseStrokeProps} /> : null}
        </>
      );
  }
}

export function GraphPinView({
  id,
  name,
  direction,
  isConnected,
  contextMenuOpen,
  validationWarning,
  dragStyle,
  connectionFeedback,
  visualSpec,
  renderStyle,
  baseColor,
  shouldPulse,
  tooltip,
  inputSlot,
  contextMenuSlot,
  onContextMenu,
  onPointerDown,
  onClick,
}: GraphPinViewProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div
          className={`group relative flex h-7 shrink-0 items-center transition-opacity pin-container ${
            direction === "input" ? "flex-row justify-start" : "flex-row-reverse justify-end"
          }`}
          style={dragStyle}
          data-pin-id={id}
          {...pinConnectionFeedbackAttributes(connectionFeedback)}
          data-validation-warning={validationWarning ? "true" : undefined}
          onContextMenu={onContextMenu}
        >
          <div
            data-pin-connection-anchor={id}
            className={`relative z-20 flex h-6 w-6 shrink-0 cursor-crosshair items-center justify-center rounded-full pin-circle ${
              direction === "input" ? "mr-1" : "ml-1"
            } ${contextMenuOpen ? "ring-2 ring-[var(--accent-color)]/60" : ""} ${
              validationWarning ? "ring-2 ring-amber-500/80" : ""
            } ${pinConnectionFeedbackClass(connectionFeedback)}`}
            onPointerDown={onPointerDown}
            onClick={onClick}
          >
            <svg
              width="12"
              height="12"
              viewBox="0 0 12 12"
              className="overflow-visible"
              style={{ display: "block" }}
            >
              <GraphPinShape
                visualSpec={visualSpec}
                renderStyle={renderStyle}
                shouldPulse={shouldPulse}
                baseColor={baseColor}
              />
              {shouldPulse ? (
                <defs>
                  <filter id="pinGlow" x="-50%" y="-50%" width="200%" height="200%">
                    <feGaussianBlur in="SourceGraphic" stdDeviation="1.5" />
                  </filter>
                </defs>
              ) : null}
              {isConnected && visualSpec.edgeKind === "data" ? (
                <circle cx="6" cy="6" r="1.2" fill="white" className="pointer-events-none" />
              ) : null}
            </svg>
          </div>

          <span
            className={`z-10 select-none px-1 text-[10px] font-bold tracking-wide transition-colors pointer-events-none ${
              contextMenuOpen
                ? "text-[var(--accent-color)]"
                : isConnected
                  ? "text-foreground"
                  : "text-muted-foreground"
            } ${contextMenuOpen ? "" : "group-hover:text-foreground"}`}
          >
            {name}
          </span>

          {inputSlot}
          {contextMenuSlot}
        </div>
      </TooltipTrigger>
      <TooltipContent side={direction === "input" ? "left" : "right"}>{tooltip}</TooltipContent>
    </Tooltip>
  );
}
